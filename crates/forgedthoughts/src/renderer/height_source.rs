use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{
    EvalState, GraphRenderSourceKind, ObjectValue, Value,
    compile_specialized_height_node_eval_function, eval_node_function, jit::JitFunction,
};
use crate::ast::{MaterialFunctionStatement, MaterialStatement};
use crate::eval::{NodeSampleProvider, with_node_sample_provider};

const FIELD_TILE_CELLS: i32 = 64;

#[derive(Clone)]
pub struct HeightSampler<'a> {
    state: &'a EvalState,
    root_obj: Arc<ObjectValue>,
    root_kind: GraphRenderSourceKind,
    point_jit_cache: Arc<Mutex<HashMap<usize, Option<JitFunction>>>>,
    slope_tile_cache: Arc<Mutex<HashMap<SlopeTileKey, Arc<SlopeTile>>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SlopeTileKey {
    node_key: usize,
    tile_x: i32,
    tile_z: i32,
}

#[derive(Debug, Clone)]
struct SlopeTile {
    origin_x: i32,
    origin_z: i32,
    cells: i32,
    values: Vec<f32>,
}

impl<'a> HeightSampler<'a> {
    pub fn new(
        state: &'a EvalState,
        root_obj: ObjectValue,
        root_kind: GraphRenderSourceKind,
    ) -> Self {
        Self {
            state,
            root_obj: Arc::new(root_obj),
            root_kind,
            point_jit_cache: Arc::new(Mutex::new(HashMap::new())),
            slope_tile_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn sample_root_scalar(&self, x: f32, z: f32) -> Option<f32> {
        let node_name = self.root_obj.type_name.as_deref()?;
        self.sample_scalar_by_kind(node_name, self.root_obj.as_ref(), self.root_kind, x, z)
    }

    fn sample_scalar_by_kind(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        source_kind: GraphRenderSourceKind,
        x: f32,
        z: f32,
    ) -> Option<f32> {
        match source_kind {
            GraphRenderSourceKind::PointScalar => {
                self.sample_point_scalar_node(node_name, graph_obj, x, z)
            }
            GraphRenderSourceKind::FieldScalar => {
                self.sample_field_scalar_node(node_name, graph_obj, x, z)
            }
        }
    }

    pub fn sample_point_scalar_node(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        x: f32,
        z: f32,
    ) -> Option<f32> {
        let jit = self.point_jit_for(node_name, graph_obj);
        jit.and_then(|compiled| compiled.invoke(&[x, z])).or_else(|| {
            let ctx = make_height_context(x, z);
            eval_node_function(
                self.state,
                node_name,
                Some(graph_obj),
                "eval",
                &[Value::Object(ctx)],
            )
            .ok()
            .and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            })
        })
    }

    pub fn sample_point_object(&self, graph_obj: &ObjectValue, x: f32, z: f32) -> Option<f32> {
        let node_name = graph_obj.type_name.as_deref()?;
        self.sample_point_scalar_node(node_name, graph_obj, x, z)
    }

    fn point_jit_for(&self, node_name: &str, graph_obj: &ObjectValue) -> Option<JitFunction> {
        let object_key = object_key(graph_obj);
        if let Some(compiled) = self
            .point_jit_cache
            .lock()
            .expect("height sampler point-jit cache poisoned")
            .get(&object_key)
            .copied()
            .flatten()
        {
            return Some(compiled);
        }

        let compiled = compile_specialized_height_node_eval_function(
            self.state,
            node_name,
            Some(graph_obj),
        );
        self.point_jit_cache
            .lock()
            .expect("height sampler point-jit cache poisoned")
            .insert(object_key, compiled);
        compiled
    }

    fn sample_field_scalar_node(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        x: f32,
        z: f32,
    ) -> Option<f32> {
        self.sample_tiled_field_node(node_name, graph_obj, x, z)
    }

    fn sample_tiled_field_node(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        x: f32,
        z: f32,
    ) -> Option<f32> {
        let step = match graph_obj.fields.get("step") {
            Some(Value::Number(v)) => (*v).max(1.0e-5),
            _ => 0.002,
        };

        let gx = x / step;
        let gz = z / step;
        let base_x = gx.floor() as i32;
        let base_z = gz.floor() as i32;
        let tile_x = div_floor(base_x, FIELD_TILE_CELLS);
        let tile_z = div_floor(base_z, FIELD_TILE_CELLS);
        let tile = self.field_tile(node_name, graph_obj, step, tile_x, tile_z)?;
        let local_x = (gx - tile.origin_x as f32).clamp(0.0, FIELD_TILE_CELLS as f32);
        let local_z = (gz - tile.origin_z as f32).clamp(0.0, FIELD_TILE_CELLS as f32);

        bilinear_tile_sample(&tile, local_x, local_z)
    }

    fn field_tile(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        step: f32,
        tile_x: i32,
        tile_z: i32,
    ) -> Option<Arc<SlopeTile>> {
        let key = SlopeTileKey {
            node_key: object_key(graph_obj),
            tile_x,
            tile_z,
        };

        if let Some(tile) = self
            .slope_tile_cache
            .lock()
            .expect("height sampler field cache poisoned")
            .get(&key)
            .cloned()
        {
            return Some(tile);
        }

        let tile = Arc::new(self.build_field_tile(node_name, graph_obj, step, tile_x, tile_z)?);
        self.slope_tile_cache
            .lock()
            .expect("height sampler field cache poisoned")
            .insert(key, tile.clone());
        Some(tile)
    }

    fn build_field_tile(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        step: f32,
        tile_x: i32,
        tile_z: i32,
    ) -> Option<SlopeTile> {
        let origin_x = tile_x * FIELD_TILE_CELLS;
        let origin_z = tile_z * FIELD_TILE_CELLS;
        let resolution = (FIELD_TILE_CELLS + 1) as usize;
        let mut values = vec![0.0; resolution * resolution];

        for local_z in 0..=FIELD_TILE_CELLS {
            for local_x in 0..=FIELD_TILE_CELLS {
                let wx = (origin_x + local_x) as f32 * step;
                let wz = (origin_z + local_z) as f32 * step;
                let value = self.eval_field_node_at(node_name, graph_obj, wx, wz)?;
                values[(local_z as usize) * resolution + local_x as usize] = value;
            }
        }

        Some(SlopeTile {
            origin_x,
            origin_z,
            cells: FIELD_TILE_CELLS,
            values,
        })
    }

    fn eval_field_node_at(
        &self,
        node_name: &str,
        graph_obj: &ObjectValue,
        x: f32,
        z: f32,
    ) -> Option<f32> {
        if self.should_use_compiled_field_eval(node_name)
            && let Some(compiled) = self.point_jit_for(node_name, graph_obj)
            && let Some(value) = compiled.invoke(&[x, z])
        {
            return Some(value);
        }
        with_node_sample_provider(self, || {
            let ctx = make_height_context(x, z);
            eval_node_function(
                self.state,
                node_name,
                Some(graph_obj),
                "eval",
                &[Value::Object(ctx)],
            )
            .ok()
            .and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            })
        })
    }

    fn should_use_compiled_field_eval(&self, node_name: &str) -> bool {
        let Some(def) = self.state.node_defs.get(node_name) else {
            return false;
        };
        let mut complexity = 0usize;
        for stmt in &def.statements {
            match stmt {
                MaterialStatement::Binding { .. } => complexity += 1,
                MaterialStatement::Property { .. } => {}
                MaterialStatement::Function { body, .. } => {
                    complexity += function_stmt_complexity(body);
                }
            }
        }
        complexity <= 48
    }
}

fn function_stmt_complexity(body: &[MaterialFunctionStatement]) -> usize {
    let mut total = 0usize;
    for stmt in body {
        total += match stmt {
            MaterialFunctionStatement::Binding { .. } => 1,
            MaterialFunctionStatement::Return { .. } => 1,
            MaterialFunctionStatement::ForLoop { body, .. } => 6 + function_stmt_complexity(body),
        };
    }
    total
}

impl<'a> NodeSampleProvider for HeightSampler<'a> {
    fn sample_point_node(&self, target: &ObjectValue, x: f32, z: f32) -> Option<f32> {
        self.sample_point_object(target, x, z)
    }
}

pub fn make_height_context(x: f32, z: f32) -> ObjectValue {
    let zero = Value::Number(0.0);
    let pos2d = Value::Object(ObjectValue {
        type_name: Some("vec3".to_string()),
        fields: std::collections::HashMap::from([
            ("x".to_string(), Value::Number(x)),
            ("y".to_string(), Value::Number(0.0)),
            ("z".to_string(), Value::Number(z)),
        ]),
    });

    ObjectValue {
        type_name: Some("NodeContext".to_string()),
        fields: std::collections::HashMap::from([
            ("stage".to_string(), Value::String("height".to_string())),
            ("pos2d".to_string(), pos2d),
            ("height".to_string(), zero.clone()),
            ("mask".to_string(), zero.clone()),
            ("value".to_string(), zero),
        ]),
    }
}

fn bilinear_tile_sample(tile: &SlopeTile, x: f32, z: f32) -> Option<f32> {
    let cells = tile.cells.max(1);
    let resolution = (cells + 1) as usize;
    let x0 = x.floor().clamp(0.0, (cells - 1) as f32) as usize;
    let z0 = z.floor().clamp(0.0, (cells - 1) as f32) as usize;
    let tx = (x - x0 as f32).clamp(0.0, 1.0);
    let tz = (z - z0 as f32).clamp(0.0, 1.0);
    let x1 = (x0 + 1).min(cells as usize);
    let z1 = (z0 + 1).min(cells as usize);

    let v00 = *tile.values.get(z0 * resolution + x0)?;
    let v10 = *tile.values.get(z0 * resolution + x1)?;
    let v01 = *tile.values.get(z1 * resolution + x0)?;
    let v11 = *tile.values.get(z1 * resolution + x1)?;

    let vx0 = v00 + (v10 - v00) * tx;
    let vx1 = v01 + (v11 - v01) * tx;
    Some(vx0 + (vx1 - vx0) * tz)
}

fn div_floor(value: i32, divisor: i32) -> i32 {
    let mut q = value / divisor;
    let r = value % divisor;
    if r != 0 && ((r > 0) != (divisor > 0)) {
        q -= 1;
    }
    q
}

fn object_key(object: &ObjectValue) -> usize {
    object as *const ObjectValue as usize
}
