use std::collections::HashMap;

use cranelift_codegen::ir::{AbiParam, InstBuilder, types};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module, default_libcall_names};

use crate::ast::{
    BinaryOp, Expr, MaterialDef, MaterialFunctionStatement, MaterialStatement, SdfDef,
    SdfFunctionStatement, SdfStatement, UnaryOp,
};
use crate::vm::{VmFunction, VmInstruction};

#[derive(Debug, Clone, Copy)]
pub struct JitFunction {
    code_ptr: *const u8,
    argc: usize,
}

#[derive(Debug, Clone)]
pub struct JitSdfDistanceFunction {
    code_ptr: *const u8,
    pub capture_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct JitModifierDistanceFunction {
    code_ptr: *const u8,
}

#[derive(Debug, Clone)]
pub struct JitSdfVec3Function {
    pub capture_names: Vec<String>,
    components: [JitFunction; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitCaptureKind {
    Scalar,
    Vec3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JitCapture {
    pub name: String,
    pub kind: JitCaptureKind,
}

#[derive(Debug, Clone, Copy)]
pub struct JitVec3Function {
    components: [JitFunction; 3],
}

unsafe impl Send for JitFunction {}
unsafe impl Sync for JitFunction {}
unsafe impl Send for JitSdfDistanceFunction {}
unsafe impl Sync for JitSdfDistanceFunction {}
unsafe impl Send for JitModifierDistanceFunction {}
unsafe impl Sync for JitModifierDistanceFunction {}
unsafe impl Send for JitSdfVec3Function {}
unsafe impl Sync for JitSdfVec3Function {}
unsafe impl Send for JitVec3Function {}
unsafe impl Sync for JitVec3Function {}

impl JitFunction {
    pub fn invoke(&self, args: &[f64]) -> Option<f64> {
        if args.len() != self.argc {
            return None;
        }
        invoke_code_ptr(self.code_ptr, args)
    }
}

impl JitSdfDistanceFunction {
    pub fn invoke(&self, p: [f64; 3], captures: &[f64]) -> Option<f64> {
        let mut args = Vec::with_capacity(3 + captures.len());
        args.extend_from_slice(&p);
        args.extend_from_slice(captures);
        invoke_code_ptr(self.code_ptr, &args)
    }
}

impl JitModifierDistanceFunction {
    pub fn invoke(&self, d: f64, p: [f64; 3]) -> Option<f64> {
        invoke_code_ptr(self.code_ptr, &[d, p[0], p[1], p[2]])
    }
}

impl JitSdfVec3Function {
    pub fn invoke(&self, p: [f64; 3], captures: &[f64]) -> Option<[f64; 3]> {
        let mut args = Vec::with_capacity(3 + captures.len());
        args.extend_from_slice(&p);
        args.extend_from_slice(captures);
        Some([
            self.components[0].invoke(&args)?,
            self.components[1].invoke(&args)?,
            self.components[2].invoke(&args)?,
        ])
    }
}

impl JitVec3Function {
    pub fn invoke(&self, args: &[f64]) -> Option<[f64; 3]> {
        Some([
            self.components[0].invoke(args)?,
            self.components[1].invoke(args)?,
            self.components[2].invoke(args)?,
        ])
    }
}

fn invoke_code_ptr(code_ptr: *const u8, args: &[f64]) -> Option<f64> {
    unsafe {
        match args.len() {
            0 => {
                let f: extern "C" fn() -> f64 = std::mem::transmute(code_ptr);
                Some(f())
            }
            1 => {
                let f: extern "C" fn(f64) -> f64 = std::mem::transmute(code_ptr);
                Some(f(args[0]))
            }
            2 => {
                let f: extern "C" fn(f64, f64) -> f64 = std::mem::transmute(code_ptr);
                Some(f(args[0], args[1]))
            }
            3 => {
                let f: extern "C" fn(f64, f64, f64) -> f64 = std::mem::transmute(code_ptr);
                Some(f(args[0], args[1], args[2]))
            }
            4 => {
                let f: extern "C" fn(f64, f64, f64, f64) -> f64 = std::mem::transmute(code_ptr);
                Some(f(args[0], args[1], args[2], args[3]))
            }
            5 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(args[0], args[1], args[2], args[3], args[4]))
            }
            6 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(args[0], args[1], args[2], args[3], args[4], args[5]))
            }
            7 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(
                    args[0], args[1], args[2], args[3], args[4], args[5], args[6],
                ))
            }
            8 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(
                    args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
                ))
            }
            9 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(
                    args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
                ))
            }
            10 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(
                    args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
                    args[8], args[9],
                ))
            }
            11 => {
                let f: extern "C" fn(f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64 =
                    std::mem::transmute(code_ptr);
                Some(f(
                    args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
                    args[8], args[9], args[10],
                ))
            }
            12 => {
                let f: extern "C" fn(
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                    f64,
                ) -> f64 = std::mem::transmute(code_ptr);
                Some(f(
                    args[0], args[1], args[2], args[3], args[4], args[5], args[6], args[7],
                    args[8], args[9], args[10], args[11],
                ))
            }
            _ => None,
        }
    }
}

fn create_module() -> Option<JITModule> {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").ok()?;
    flag_builder.set("is_pic", "false").ok()?;
    let isa_builder = cranelift_native::builder().ok()?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .ok()?;
    let builder = JITBuilder::with_isa(isa, default_libcall_names());
    Some(JITModule::new(builder))
}

pub fn compile_jit_function(name: &str, function: &VmFunction) -> Option<JitFunction> {
    if function.params.len() > 12 {
        return None;
    }

    let mut module = create_module()?;
    let mut sig = module.make_signature();
    for _ in &function.params {
        sig.params.push(AbiParam::new(types::F64));
    }
    sig.returns.push(AbiParam::new(types::F64));

    let func_id = module.declare_function(name, Linkage::Local, &sig).ok()?;

    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    ctx.func.signature.call_conv = CallConv::triple_default(module.isa().triple());

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let mut vars = HashMap::new();
    for (index, param) in function.params.iter().enumerate() {
        let var = Variable::from_u32(index as u32);
        fb.declare_var(var, types::F64);
        let value = fb.block_params(block)[index];
        fb.def_var(var, value);
        vars.insert(param.clone(), var);
    }

    let mut stack: Vec<cranelift_codegen::ir::Value> = Vec::new();
    let mut next_var = function.params.len();

    for instruction in &function.code {
        match instruction {
            VmInstruction::PushNumber(v) => stack.push(fb.ins().f64const(*v)),
            VmInstruction::LoadName(name) => {
                let var = *vars.get(name)?;
                stack.push(fb.use_var(var));
            }
            VmInstruction::Unary(UnaryOp::Neg) => {
                let value = stack.pop()?;
                stack.push(fb.ins().fneg(value));
            }
            VmInstruction::Binary(op) => {
                let rhs = stack.pop()?;
                let lhs = stack.pop()?;
                let out = match op {
                    BinaryOp::Add => fb.ins().fadd(lhs, rhs),
                    BinaryOp::Sub => fb.ins().fsub(lhs, rhs),
                    BinaryOp::Mul => fb.ins().fmul(lhs, rhs),
                    BinaryOp::Div => fb.ins().fdiv(lhs, rhs),
                    BinaryOp::Intersect => return None,
                };
                stack.push(out);
            }
            VmInstruction::CallNamed { name, argc } => {
                if stack.len() < *argc {
                    return None;
                }
                let start = stack.len() - *argc;
                let args = stack.drain(start..).collect::<Vec<_>>();
                let value = emit_builtin_call(&mut fb, &mut module, name, &args)?;
                stack.push(value);
            }
            VmInstruction::StoreLocal(name) => {
                let value = stack.pop()?;
                let var = Variable::from_u32(next_var as u32);
                next_var += 1;
                fb.declare_var(var, types::F64);
                fb.def_var(var, value);
                vars.insert(name.clone(), var);
            }
            VmInstruction::Return => {
                let value = stack.pop()?;
                fb.ins().return_(&[value]);
            }
            VmInstruction::PushString(_)
            | VmInstruction::BuildArray(_)
            | VmInstruction::BuildObject { .. }
            | VmInstruction::LoadMember(_) => return None,
        }
    }

    finalize_scalar_function(module, func_id, ctx, function.params.len())
}

#[derive(Clone, Copy)]
enum SdfJitValue {
    Scalar(cranelift_codegen::ir::Value),
    Vec3([cranelift_codegen::ir::Value; 3]),
}

struct SdfJitContext<'a, 'b> {
    fb: &'a mut FunctionBuilder<'b>,
    module: &'a mut JITModule,
    locals: HashMap<String, SdfJitValue>,
    functions: &'a HashMap<String, (Vec<String>, Vec<SdfFunctionStatement>)>,
    captures: &'a HashMap<String, Variable>,
}

pub fn compile_sdf_distance_function(def: &SdfDef) -> Option<JitSdfDistanceFunction> {
    let (params, body) = def.statements.iter().find_map(|stmt| match stmt {
        SdfStatement::Function { name, params, body }
            if name == "distance" && params.len() == 1 =>
        {
            Some((params.clone(), body.clone()))
        }
        _ => None,
    })?;

    let top_level_bindings = def
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            SdfStatement::Binding { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let functions = def
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            SdfStatement::Function { name, params, body } => {
                Some((name.clone(), (params.clone(), body.clone())))
            }
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let capture_names =
        collect_sdf_distance_captures(&params, &body, &top_level_bindings, &functions);
    if capture_names.len() + 3 > 12 {
        return None;
    }

    let mut module = create_module()?;
    let mut sig = module.make_signature();
    for _ in 0..(3 + capture_names.len()) {
        sig.params.push(AbiParam::new(types::F64));
    }
    sig.returns.push(AbiParam::new(types::F64));

    let func_id = module
        .declare_function(&format!("{}_distance_vec3", def.name), Linkage::Local, &sig)
        .ok()?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    ctx.func.signature.call_conv = CallConv::triple_default(module.isa().triple());

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let block_params = fb.block_params(block).to_vec();
    let mut locals = HashMap::new();
    locals.insert(
        params[0].clone(),
        SdfJitValue::Vec3([block_params[0], block_params[1], block_params[2]]),
    );

    let mut captures = HashMap::new();
    for (index, name) in capture_names.iter().enumerate() {
        let var = Variable::from_u32(index as u32);
        fb.declare_var(var, types::F64);
        fb.def_var(var, block_params[3 + index]);
        captures.insert(name.clone(), var);
    }

    let mut jit_ctx = SdfJitContext {
        fb: &mut fb,
        module: &mut module,
        locals,
        functions: &functions,
        captures: &captures,
    };

    for stmt in &body {
        match stmt {
            SdfFunctionStatement::Binding { name, expr } => {
                let value = compile_sdf_expr(expr, &mut jit_ctx)?;
                jit_ctx.locals.insert(name.clone(), value);
            }
            SdfFunctionStatement::Return { expr } => {
                let value = compile_sdf_expr(expr, &mut jit_ctx)?;
                let SdfJitValue::Scalar(value) = value else {
                    return None;
                };
                jit_ctx.fb.ins().return_(&[value]);
            }
        }
    }

    fb.finalize();
    module.define_function(func_id, &mut ctx).ok()?;
    module.clear_context(&mut ctx);
    module.finalize_definitions().ok()?;
    let code_ptr = module.get_finalized_function(func_id);
    let _leaked_module = Box::leak(Box::new(module));
    Some(JitSdfDistanceFunction {
        code_ptr,
        capture_names,
    })
}

pub fn compile_sdf_vec3_function(def: &SdfDef, function_name: &str) -> Option<JitSdfVec3Function> {
    let (params, body) = def.statements.iter().find_map(|stmt| match stmt {
        SdfStatement::Function { name, params, body }
            if name == function_name && params.len() == 1 =>
        {
            Some((params.clone(), body.clone()))
        }
        _ => None,
    })?;

    let top_level_bindings = def
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            SdfStatement::Binding { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let functions = def
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            SdfStatement::Function { name, params, body } => {
                Some((name.clone(), (params.clone(), body.clone())))
            }
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let capture_names =
        collect_sdf_distance_captures(&params, &body, &top_level_bindings, &functions);
    if capture_names.len() + 3 > 12 {
        return None;
    }

    let x = compile_sdf_vec3_component(
        def,
        function_name,
        &params,
        &body,
        &functions,
        &capture_names,
        0,
    )?;
    let y = compile_sdf_vec3_component(
        def,
        function_name,
        &params,
        &body,
        &functions,
        &capture_names,
        1,
    )?;
    let z = compile_sdf_vec3_component(
        def,
        function_name,
        &params,
        &body,
        &functions,
        &capture_names,
        2,
    )?;

    Some(JitSdfVec3Function {
        capture_names,
        components: [x, y, z],
    })
}

pub fn compile_modifier_distance_function(
    name: &str,
    params: &[String],
    body: &[SdfFunctionStatement],
) -> Option<JitModifierDistanceFunction> {
    if params.len() != 2 {
        return None;
    }
    let functions = HashMap::from([(name.to_string(), (params.to_vec(), body.to_vec()))]);
    let capture_names: Vec<String> = Vec::new();
    let mut module = create_module()?;
    let mut sig = module.make_signature();
    for _ in 0..4 {
        sig.params.push(AbiParam::new(types::F64));
    }
    sig.returns.push(AbiParam::new(types::F64));

    let func_id = module
        .declare_function(&format!("{name}_modifier_distance"), Linkage::Local, &sig)
        .ok()?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    ctx.func.signature.call_conv = CallConv::triple_default(module.isa().triple());

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let block_params = fb.block_params(block).to_vec();
    let mut locals = HashMap::new();
    locals.insert(params[0].clone(), SdfJitValue::Scalar(block_params[0]));
    locals.insert(
        params[1].clone(),
        SdfJitValue::Vec3([block_params[1], block_params[2], block_params[3]]),
    );
    let captures = HashMap::new();

    let mut jit_ctx = SdfJitContext {
        fb: &mut fb,
        module: &mut module,
        locals,
        functions: &functions,
        captures: &captures,
    };

    for stmt in body {
        match stmt {
            SdfFunctionStatement::Binding { name, expr } => {
                let value = compile_sdf_expr(expr, &mut jit_ctx)?;
                jit_ctx.locals.insert(name.clone(), value);
            }
            SdfFunctionStatement::Return { expr } => {
                let value = compile_sdf_expr(expr, &mut jit_ctx)?;
                let SdfJitValue::Scalar(value) = value else {
                    return None;
                };
                jit_ctx.fb.ins().return_(&[value]);
            }
        }
    }

    fb.finalize();
    module.define_function(func_id, &mut ctx).ok()?;
    module.clear_context(&mut ctx);
    module.finalize_definitions().ok()?;
    let code_ptr = module.get_finalized_function(func_id);
    let _leaked_module = Box::leak(Box::new(module));
    let _ = capture_names;
    Some(JitModifierDistanceFunction { code_ptr })
}

#[derive(Clone, Copy)]
enum MaterialJitValue {
    Scalar(cranelift_codegen::ir::Value),
    Vec3([cranelift_codegen::ir::Value; 3]),
}

struct MaterialJitContext<'a, 'b> {
    fb: &'a mut FunctionBuilder<'b>,
    module: &'a mut JITModule,
    locals: HashMap<String, MaterialJitValue>,
    functions: &'a HashMap<String, (Vec<String>, Vec<MaterialFunctionStatement>)>,
    captures: &'a HashMap<String, MaterialJitValue>,
}

pub fn compile_material_vec3_function(
    def: &MaterialDef,
    function_name: &str,
) -> Option<(Vec<JitCapture>, JitVec3Function)> {
    let (params, body) = def.statements.iter().find_map(|stmt| match stmt {
        MaterialStatement::Function { name, params, body } if name == function_name => {
            Some((params.clone(), body.clone()))
        }
        _ => None,
    })?;
    if params.len() != 1 {
        return None;
    }

    let top_level_bindings = infer_material_binding_kinds(def);
    let functions = def
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            MaterialStatement::Function { name, params, body } => {
                Some((name.clone(), (params.clone(), body.clone())))
            }
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let captures = collect_material_vec3_captures(&params, &body, &top_level_bindings, &functions);
    let argc = captures
        .iter()
        .map(|capture| match capture.kind {
            JitCaptureKind::Scalar => 1,
            JitCaptureKind::Vec3 => 3,
        })
        .sum::<usize>();
    if argc > 12 {
        return None;
    }
    if !material_function_returns_vec3(&body, &functions, &captures, argc)? {
        return None;
    }

    let x =
        compile_material_vec3_component(def, function_name, &body, &functions, &captures, argc, 0)?;
    let y =
        compile_material_vec3_component(def, function_name, &body, &functions, &captures, argc, 1)?;
    let z =
        compile_material_vec3_component(def, function_name, &body, &functions, &captures, argc, 2)?;
    Some((
        captures,
        JitVec3Function {
            components: [x, y, z],
        },
    ))
}

fn material_function_returns_vec3(
    body: &[MaterialFunctionStatement],
    functions: &HashMap<String, (Vec<String>, Vec<MaterialFunctionStatement>)>,
    captures: &[JitCapture],
    argc: usize,
) -> Option<bool> {
    let mut module = create_module()?;
    let mut sig = module.make_signature();
    for _ in 0..argc {
        sig.params.push(AbiParam::new(types::F64));
    }
    sig.returns.push(AbiParam::new(types::F64));

    let func_id = module
        .declare_function("material_vec3_probe", Linkage::Local, &sig)
        .ok()?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    ctx.func.signature.call_conv = CallConv::triple_default(module.isa().triple());

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let block_params = fb.block_params(block).to_vec();
    let mut capture_vars = HashMap::new();
    let mut next_param = 0usize;
    for capture in captures {
        match capture.kind {
            JitCaptureKind::Scalar => {
                capture_vars.insert(
                    capture.name.clone(),
                    MaterialJitValue::Scalar(block_params[next_param]),
                );
                next_param += 1;
            }
            JitCaptureKind::Vec3 => {
                capture_vars.insert(
                    capture.name.clone(),
                    MaterialJitValue::Vec3([
                        block_params[next_param],
                        block_params[next_param + 1],
                        block_params[next_param + 2],
                    ]),
                );
                next_param += 3;
            }
        }
    }

    let mut jit_ctx = MaterialJitContext {
        fb: &mut fb,
        module: &mut module,
        locals: HashMap::new(),
        functions,
        captures: &capture_vars,
    };
    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = compile_material_expr(expr, &mut jit_ctx)?;
                jit_ctx.locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                let value = compile_material_expr(expr, &mut jit_ctx)?;
                return Some(matches!(value, MaterialJitValue::Vec3(_)));
            }
        }
    }
    let _ = func_id;
    None
}

fn compile_material_vec3_component(
    def: &MaterialDef,
    function_name: &str,
    body: &[MaterialFunctionStatement],
    functions: &HashMap<String, (Vec<String>, Vec<MaterialFunctionStatement>)>,
    captures: &[JitCapture],
    argc: usize,
    component: usize,
) -> Option<JitFunction> {
    let mut module = create_module()?;
    let mut sig = module.make_signature();
    for _ in 0..argc {
        sig.params.push(AbiParam::new(types::F64));
    }
    sig.returns.push(AbiParam::new(types::F64));

    let func_id = module
        .declare_function(
            &format!("{}_{}_vec3_{component}", def.name, function_name),
            Linkage::Local,
            &sig,
        )
        .ok()?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    ctx.func.signature.call_conv = CallConv::triple_default(module.isa().triple());

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let block_params = fb.block_params(block).to_vec();
    let mut capture_vars = HashMap::new();
    let mut next_param = 0usize;
    for capture in captures {
        match capture.kind {
            JitCaptureKind::Scalar => {
                let var = Variable::from_u32(next_param as u32);
                fb.declare_var(var, types::F64);
                fb.def_var(var, block_params[next_param]);
                capture_vars.insert(
                    capture.name.clone(),
                    MaterialJitValue::Scalar(fb.use_var(var)),
                );
                next_param += 1;
            }
            JitCaptureKind::Vec3 => {
                let mut vec = [fb.ins().f64const(0.0); 3];
                for axis in 0..3 {
                    let var = Variable::from_u32((next_param + axis) as u32);
                    fb.declare_var(var, types::F64);
                    fb.def_var(var, block_params[next_param + axis]);
                    vec[axis] = fb.use_var(var);
                }
                capture_vars.insert(capture.name.clone(), MaterialJitValue::Vec3(vec));
                next_param += 3;
            }
        }
    }

    let mut jit_ctx = MaterialJitContext {
        fb: &mut fb,
        module: &mut module,
        locals: HashMap::new(),
        functions,
        captures: &capture_vars,
    };
    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = compile_material_expr(expr, &mut jit_ctx)?;
                jit_ctx.locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                let value = compile_material_expr(expr, &mut jit_ctx)?;
                let component_value = match value {
                    MaterialJitValue::Scalar(value) => value,
                    MaterialJitValue::Vec3(values) => values[component],
                };
                jit_ctx.fb.ins().return_(&[component_value]);
            }
        }
    }
    fb.finalize();
    finalize_scalar_function(module, func_id, ctx, argc)
}

fn finalize_scalar_function(
    mut module: JITModule,
    func_id: cranelift_module::FuncId,
    mut ctx: cranelift_codegen::Context,
    argc: usize,
) -> Option<JitFunction> {
    module.define_function(func_id, &mut ctx).ok()?;
    module.clear_context(&mut ctx);
    module.finalize_definitions().ok()?;
    let code_ptr = module.get_finalized_function(func_id);
    let _leaked_module = Box::leak(Box::new(module));
    Some(JitFunction { code_ptr, argc })
}

fn collect_sdf_distance_captures(
    params: &[String],
    body: &[SdfFunctionStatement],
    top_level_bindings: &[String],
    functions: &HashMap<String, (Vec<String>, Vec<SdfFunctionStatement>)>,
) -> Vec<String> {
    fn collect_expr(
        expr: &Expr,
        locals: &[String],
        captures: &mut Vec<String>,
        top_level_bindings: &[String],
        functions: &HashMap<String, (Vec<String>, Vec<SdfFunctionStatement>)>,
        seen_functions: &mut Vec<String>,
    ) {
        match expr {
            Expr::Number(_) | Expr::String(_) => {}
            Expr::Array(items) => {
                for item in items {
                    collect_expr(
                        item,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::Ident(name) => {
                if !locals.iter().any(|local| local == name)
                    && top_level_bindings.iter().any(|binding| binding == name)
                    && !captures.iter().any(|capture| capture == name)
                {
                    captures.push(name.clone());
                }
            }
            Expr::ObjectLiteral { fields, .. } => {
                for (_, value) in fields {
                    collect_expr(
                        value,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                collect_expr(
                    lhs,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
                collect_expr(
                    rhs,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
            }
            Expr::Member { target, .. } => {
                collect_expr(
                    target,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
            }
            Expr::Unary { expr, .. } => {
                collect_expr(
                    expr,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
            }
            Expr::Call { callee, args } => {
                if let Expr::Ident(name) = callee.as_ref()
                    && let Some((params, body)) = functions.get(name)
                    && !seen_functions.iter().any(|seen| seen == name)
                {
                    seen_functions.push(name.clone());
                    let mut fn_locals = locals.to_vec();
                    fn_locals.extend(params.iter().cloned());
                    for stmt in body {
                        match stmt {
                            SdfFunctionStatement::Binding { name, expr } => {
                                collect_expr(
                                    expr,
                                    &fn_locals,
                                    captures,
                                    top_level_bindings,
                                    functions,
                                    seen_functions,
                                );
                                fn_locals.push(name.clone());
                            }
                            SdfFunctionStatement::Return { expr } => {
                                collect_expr(
                                    expr,
                                    &fn_locals,
                                    captures,
                                    top_level_bindings,
                                    functions,
                                    seen_functions,
                                );
                            }
                        }
                    }
                    seen_functions.pop();
                }
                for arg in args {
                    collect_expr(
                        arg,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::FunctionLiteral { .. } => {}
        }
    }

    let mut captures = Vec::new();
    let mut locals = params.to_vec();
    let mut seen_functions = Vec::new();
    for stmt in body {
        match stmt {
            SdfFunctionStatement::Binding { name, expr } => {
                collect_expr(
                    expr,
                    &locals,
                    &mut captures,
                    top_level_bindings,
                    functions,
                    &mut seen_functions,
                );
                locals.push(name.clone());
            }
            SdfFunctionStatement::Return { expr } => {
                collect_expr(
                    expr,
                    &locals,
                    &mut captures,
                    top_level_bindings,
                    functions,
                    &mut seen_functions,
                );
            }
        }
    }
    captures.sort();
    captures.dedup();
    captures
}

fn compile_sdf_expr(expr: &Expr, ctx: &mut SdfJitContext<'_, '_>) -> Option<SdfJitValue> {
    match expr {
        Expr::Number(value) => Some(SdfJitValue::Scalar(ctx.fb.ins().f64const(*value))),
        Expr::Ident(name) => {
            if let Some(value) = ctx.locals.get(name) {
                return Some(*value);
            }
            if let Some(var) = ctx.captures.get(name) {
                return Some(SdfJitValue::Scalar(ctx.fb.use_var(*var)));
            }
            None
        }
        Expr::Member { target, field } => {
            let value = compile_sdf_expr(target, ctx)?;
            let SdfJitValue::Vec3(vec) = value else {
                return None;
            };
            match field.as_str() {
                "x" => Some(SdfJitValue::Scalar(vec[0])),
                "y" => Some(SdfJitValue::Scalar(vec[1])),
                "z" => Some(SdfJitValue::Scalar(vec[2])),
                _ => None,
            }
        }
        Expr::ObjectLiteral { type_name, fields } if type_name == "vec3" => {
            let zero = ctx.fb.ins().f64const(0.0);
            let mut values = [zero, zero, zero];
            for (name, expr) in fields {
                let SdfJitValue::Scalar(value) = compile_sdf_expr(expr, ctx)? else {
                    return None;
                };
                match name.as_str() {
                    "x" => values[0] = value,
                    "y" => values[1] = value,
                    "z" => values[2] = value,
                    _ => return None,
                }
            }
            Some(SdfJitValue::Vec3(values))
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => match compile_sdf_expr(expr, ctx)? {
            SdfJitValue::Scalar(value) => Some(SdfJitValue::Scalar(ctx.fb.ins().fneg(value))),
            SdfJitValue::Vec3(values) => Some(SdfJitValue::Vec3([
                ctx.fb.ins().fneg(values[0]),
                ctx.fb.ins().fneg(values[1]),
                ctx.fb.ins().fneg(values[2]),
            ])),
        },
        Expr::Binary { lhs, op, rhs } => {
            let lhs = compile_sdf_expr(lhs, ctx)?;
            let rhs = compile_sdf_expr(rhs, ctx)?;
            compile_sdf_binary(*op, lhs, rhs, ctx)
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = callee.as_ref() {
                if let Some((params, body)) = ctx.functions.get(name).cloned() {
                    let arg_values = args
                        .iter()
                        .map(|arg| compile_sdf_expr(arg, ctx))
                        .collect::<Option<Vec<_>>>()?;
                    return compile_inline_sdf_function(&params, &body, arg_values, ctx);
                }
                let arg_values = args
                    .iter()
                    .map(|arg| compile_sdf_expr(arg, ctx))
                    .collect::<Option<Vec<_>>>()?;
                return compile_sdf_builtin(name, &arg_values, ctx);
            }
            None
        }
        _ => None,
    }
}

fn compile_inline_sdf_function(
    params: &[String],
    body: &[SdfFunctionStatement],
    args: Vec<SdfJitValue>,
    ctx: &mut SdfJitContext<'_, '_>,
) -> Option<SdfJitValue> {
    let old_locals = ctx.locals.clone();
    for (param, value) in params.iter().zip(args.into_iter()) {
        ctx.locals.insert(param.clone(), value);
    }
    let mut result = None;
    for stmt in body {
        match stmt {
            SdfFunctionStatement::Binding { name, expr } => {
                let value = compile_sdf_expr(expr, ctx)?;
                ctx.locals.insert(name.clone(), value);
            }
            SdfFunctionStatement::Return { expr } => {
                result = Some(compile_sdf_expr(expr, ctx)?);
                break;
            }
        }
    }
    ctx.locals = old_locals;
    result
}

fn emit_rotate_vec3(
    ctx: &mut SdfJitContext<'_, '_>,
    v: [cranelift_codegen::ir::Value; 3],
    deg: cranelift_codegen::ir::Value,
    axis: u8,
) -> Option<[cranelift_codegen::ir::Value; 3]> {
    let pi_over_180 = ctx.fb.ins().f64const(std::f64::consts::PI / 180.0);
    let radians = ctx.fb.ins().fmul(deg, pi_over_180);
    let s = emit_unary_import_call(ctx.fb, ctx.module, "sin", radians)?;
    let c = emit_unary_import_call(ctx.fb, ctx.module, "cos", radians)?;
    match axis {
        0 => {
            let cy = ctx.fb.ins().fmul(c, v[1]);
            let sz = ctx.fb.ins().fmul(s, v[2]);
            let sy = ctx.fb.ins().fmul(s, v[1]);
            let cz = ctx.fb.ins().fmul(c, v[2]);
            Some([v[0], ctx.fb.ins().fsub(cy, sz), ctx.fb.ins().fadd(sy, cz)])
        }
        1 => {
            let cx = ctx.fb.ins().fmul(c, v[0]);
            let sz = ctx.fb.ins().fmul(s, v[2]);
            let sx = ctx.fb.ins().fmul(s, v[0]);
            let cz = ctx.fb.ins().fmul(c, v[2]);
            Some([ctx.fb.ins().fadd(cx, sz), v[1], ctx.fb.ins().fsub(cz, sx)])
        }
        _ => {
            let cx = ctx.fb.ins().fmul(c, v[0]);
            let sy = ctx.fb.ins().fmul(s, v[1]);
            let sx = ctx.fb.ins().fmul(s, v[0]);
            let cy = ctx.fb.ins().fmul(c, v[1]);
            Some([ctx.fb.ins().fsub(cx, sy), ctx.fb.ins().fadd(sx, cy), v[2]])
        }
    }
}

fn compile_sdf_vec3_component(
    def: &SdfDef,
    function_name: &str,
    params: &[String],
    body: &[SdfFunctionStatement],
    functions: &HashMap<String, (Vec<String>, Vec<SdfFunctionStatement>)>,
    capture_names: &[String],
    component: usize,
) -> Option<JitFunction> {
    let argc = 3 + capture_names.len();
    let mut module = create_module()?;
    let mut sig = module.make_signature();
    for _ in 0..argc {
        sig.params.push(AbiParam::new(types::F64));
    }
    sig.returns.push(AbiParam::new(types::F64));

    let func_id = module
        .declare_function(
            &format!("{}_{}_vec3_{component}", def.name, function_name),
            Linkage::Local,
            &sig,
        )
        .ok()?;
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    ctx.func.signature.call_conv = CallConv::triple_default(module.isa().triple());

    let mut builder_ctx = FunctionBuilderContext::new();
    let mut fb = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
    let block = fb.create_block();
    fb.append_block_params_for_function_params(block);
    fb.switch_to_block(block);
    fb.seal_block(block);

    let block_params = fb.block_params(block).to_vec();
    let mut locals = HashMap::new();
    locals.insert(
        params[0].clone(),
        SdfJitValue::Vec3([block_params[0], block_params[1], block_params[2]]),
    );

    let mut captures = HashMap::new();
    for (index, name) in capture_names.iter().enumerate() {
        let var = Variable::from_u32(index as u32);
        fb.declare_var(var, types::F64);
        fb.def_var(var, block_params[3 + index]);
        captures.insert(name.clone(), var);
    }

    let mut jit_ctx = SdfJitContext {
        fb: &mut fb,
        module: &mut module,
        locals,
        functions,
        captures: &captures,
    };
    for stmt in body {
        match stmt {
            SdfFunctionStatement::Binding { name, expr } => {
                let value = compile_sdf_expr(expr, &mut jit_ctx)?;
                jit_ctx.locals.insert(name.clone(), value);
            }
            SdfFunctionStatement::Return { expr } => {
                let value = compile_sdf_expr(expr, &mut jit_ctx)?;
                let component_value = match value {
                    SdfJitValue::Scalar(value) => value,
                    SdfJitValue::Vec3(values) => values[component],
                };
                jit_ctx.fb.ins().return_(&[component_value]);
            }
        }
    }

    fb.finalize();
    finalize_scalar_function(module, func_id, ctx, argc)
}

fn compile_sdf_binary(
    op: BinaryOp,
    lhs: SdfJitValue,
    rhs: SdfJitValue,
    ctx: &mut SdfJitContext<'_, '_>,
) -> Option<SdfJitValue> {
    let scalar = |ctx: &mut SdfJitContext<'_, '_>,
                  lhs: cranelift_codegen::ir::Value,
                  rhs: cranelift_codegen::ir::Value|
     -> Option<cranelift_codegen::ir::Value> {
        Some(match op {
            BinaryOp::Add => ctx.fb.ins().fadd(lhs, rhs),
            BinaryOp::Sub => ctx.fb.ins().fsub(lhs, rhs),
            BinaryOp::Mul => ctx.fb.ins().fmul(lhs, rhs),
            BinaryOp::Div => ctx.fb.ins().fdiv(lhs, rhs),
            BinaryOp::Intersect => return None,
        })
    };

    match (lhs, rhs) {
        (SdfJitValue::Scalar(lhs), SdfJitValue::Scalar(rhs)) => {
            Some(SdfJitValue::Scalar(scalar(ctx, lhs, rhs)?))
        }
        (SdfJitValue::Vec3(lhs), SdfJitValue::Vec3(rhs)) => Some(SdfJitValue::Vec3([
            scalar(ctx, lhs[0], rhs[0])?,
            scalar(ctx, lhs[1], rhs[1])?,
            scalar(ctx, lhs[2], rhs[2])?,
        ])),
        (SdfJitValue::Vec3(lhs), SdfJitValue::Scalar(rhs)) => Some(SdfJitValue::Vec3([
            scalar(ctx, lhs[0], rhs)?,
            scalar(ctx, lhs[1], rhs)?,
            scalar(ctx, lhs[2], rhs)?,
        ])),
        (SdfJitValue::Scalar(lhs), SdfJitValue::Vec3(rhs)) => Some(SdfJitValue::Vec3([
            scalar(ctx, lhs, rhs[0])?,
            scalar(ctx, lhs, rhs[1])?,
            scalar(ctx, lhs, rhs[2])?,
        ])),
    }
}

fn compile_sdf_builtin(
    name: &str,
    args: &[SdfJitValue],
    ctx: &mut SdfJitContext<'_, '_>,
) -> Option<SdfJitValue> {
    match (name, args) {
        ("vec3", [SdfJitValue::Scalar(v)]) => Some(SdfJitValue::Vec3([*v, *v, *v])),
        (
            "vec3",
            [
                SdfJitValue::Scalar(x),
                SdfJitValue::Scalar(y),
                SdfJitValue::Scalar(z),
            ],
        ) => Some(SdfJitValue::Vec3([*x, *y, *z])),
        ("abs", [SdfJitValue::Scalar(v)]) => Some(SdfJitValue::Scalar(ctx.fb.ins().fabs(*v))),
        ("abs", [SdfJitValue::Vec3(values)]) => Some(SdfJitValue::Vec3([
            ctx.fb.ins().fabs(values[0]),
            ctx.fb.ins().fabs(values[1]),
            ctx.fb.ins().fabs(values[2]),
        ])),
        ("min", [lhs, rhs]) => compile_min_max(lhs, rhs, ctx, false),
        ("max", [lhs, rhs]) => compile_min_max(lhs, rhs, ctx, true),
        ("length", [SdfJitValue::Vec3(values)]) => {
            let x2 = ctx.fb.ins().fmul(values[0], values[0]);
            let y2 = ctx.fb.ins().fmul(values[1], values[1]);
            let z2 = ctx.fb.ins().fmul(values[2], values[2]);
            let sum_xy = ctx.fb.ins().fadd(x2, y2);
            let sum = ctx.fb.ins().fadd(sum_xy, z2);
            Some(SdfJitValue::Scalar(ctx.fb.ins().sqrt(sum)))
        }
        (
            "clamp",
            [
                SdfJitValue::Scalar(x),
                SdfJitValue::Scalar(a),
                SdfJitValue::Scalar(b),
            ],
        ) => {
            let maxed = ctx.fb.ins().fmax(*x, *a);
            Some(SdfJitValue::Scalar(ctx.fb.ins().fmin(maxed, *b)))
        }
        ("rotate_x", [SdfJitValue::Vec3(v), SdfJitValue::Scalar(deg)]) => {
            Some(SdfJitValue::Vec3(emit_rotate_vec3(ctx, *v, *deg, 0)?))
        }
        ("rotate_y", [SdfJitValue::Vec3(v), SdfJitValue::Scalar(deg)]) => {
            Some(SdfJitValue::Vec3(emit_rotate_vec3(ctx, *v, *deg, 1)?))
        }
        ("rotate_z", [SdfJitValue::Vec3(v), SdfJitValue::Scalar(deg)]) => {
            Some(SdfJitValue::Vec3(emit_rotate_vec3(ctx, *v, *deg, 2)?))
        }
        _ => None,
    }
}

fn compile_min_max(
    lhs: &SdfJitValue,
    rhs: &SdfJitValue,
    ctx: &mut SdfJitContext<'_, '_>,
    is_max: bool,
) -> Option<SdfJitValue> {
    let scalar = |ctx: &mut SdfJitContext<'_, '_>,
                  lhs: cranelift_codegen::ir::Value,
                  rhs: cranelift_codegen::ir::Value|
     -> cranelift_codegen::ir::Value {
        if is_max {
            ctx.fb.ins().fmax(lhs, rhs)
        } else {
            ctx.fb.ins().fmin(lhs, rhs)
        }
    };
    match (*lhs, *rhs) {
        (SdfJitValue::Scalar(lhs), SdfJitValue::Scalar(rhs)) => {
            Some(SdfJitValue::Scalar(scalar(ctx, lhs, rhs)))
        }
        (SdfJitValue::Vec3(lhs), SdfJitValue::Vec3(rhs)) => Some(SdfJitValue::Vec3([
            scalar(ctx, lhs[0], rhs[0]),
            scalar(ctx, lhs[1], rhs[1]),
            scalar(ctx, lhs[2], rhs[2]),
        ])),
        (SdfJitValue::Vec3(lhs), SdfJitValue::Scalar(rhs)) => Some(SdfJitValue::Vec3([
            scalar(ctx, lhs[0], rhs),
            scalar(ctx, lhs[1], rhs),
            scalar(ctx, lhs[2], rhs),
        ])),
        (SdfJitValue::Scalar(lhs), SdfJitValue::Vec3(rhs)) => Some(SdfJitValue::Vec3([
            scalar(ctx, lhs, rhs[0]),
            scalar(ctx, lhs, rhs[1]),
            scalar(ctx, lhs, rhs[2]),
        ])),
    }
}

fn infer_material_binding_kinds(def: &MaterialDef) -> HashMap<String, JitCaptureKind> {
    let mut kinds = HashMap::new();
    for stmt in &def.statements {
        if let MaterialStatement::Binding { name, expr } = stmt
            && let Some(kind) = infer_material_expr_kind(expr, &kinds)
        {
            kinds.insert(name.clone(), kind);
        }
    }
    kinds
}

fn infer_material_expr_kind(
    expr: &Expr,
    known: &HashMap<String, JitCaptureKind>,
) -> Option<JitCaptureKind> {
    match expr {
        Expr::Number(_) => Some(JitCaptureKind::Scalar),
        Expr::Ident(name) => known.get(name).copied(),
        Expr::ObjectLiteral { type_name, .. } if type_name == "vec3" => Some(JitCaptureKind::Vec3),
        Expr::Unary { expr, .. } => infer_material_expr_kind(expr, known),
        Expr::Binary { lhs, rhs, .. } => match (
            infer_material_expr_kind(lhs, known),
            infer_material_expr_kind(rhs, known),
        ) {
            (Some(JitCaptureKind::Vec3), _) | (_, Some(JitCaptureKind::Vec3)) => {
                Some(JitCaptureKind::Vec3)
            }
            (Some(JitCaptureKind::Scalar), Some(JitCaptureKind::Scalar)) => {
                Some(JitCaptureKind::Scalar)
            }
            _ => None,
        },
        Expr::Member { target, field } => {
            if matches!(field.as_str(), "x" | "y" | "z") {
                Some(JitCaptureKind::Scalar)
            } else {
                infer_material_expr_kind(target, known)
            }
        }
        Expr::Call { callee, args } => {
            let Expr::Ident(name) = callee.as_ref() else {
                return None;
            };
            match name.as_str() {
                "vec3" | "normalize" | "rotate_x" | "rotate_y" | "rotate_z" => {
                    Some(JitCaptureKind::Vec3)
                }
                "length" | "step" | "smoothstep" | "sin" | "cos" | "floor" | "ceil" | "sqrt" => {
                    Some(JitCaptureKind::Scalar)
                }
                "abs" | "min" | "max" | "clamp" | "mix" => {
                    if args
                        .iter()
                        .filter_map(|arg| infer_material_expr_kind(arg, known))
                        .any(|kind| kind == JitCaptureKind::Vec3)
                    {
                        Some(JitCaptureKind::Vec3)
                    } else {
                        Some(JitCaptureKind::Scalar)
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn collect_material_vec3_captures(
    params: &[String],
    body: &[MaterialFunctionStatement],
    top_level_bindings: &HashMap<String, JitCaptureKind>,
    functions: &HashMap<String, (Vec<String>, Vec<MaterialFunctionStatement>)>,
) -> Vec<JitCapture> {
    fn known_ctx_capture_kind(name: &str) -> Option<JitCaptureKind> {
        match name {
            "ctx.local_position" | "ctx.normal" | "ctx.position" | "ctx.view_dir" => {
                Some(JitCaptureKind::Vec3)
            }
            "ctx.current_ior" | "ctx.thin_walled" => Some(JitCaptureKind::Scalar),
            _ => None,
        }
    }

    fn known_capture_kind(
        name: &str,
        top_level_bindings: &HashMap<String, JitCaptureKind>,
    ) -> Option<JitCaptureKind> {
        if let Some(kind) = known_ctx_capture_kind(name) {
            return Some(kind);
        }
        if let Some(kind) = top_level_bindings.get(name) {
            return Some(*kind);
        }
        let (base, suffix) = name.rsplit_once('.')?;
        if matches!(suffix, "x" | "y" | "z")
            && top_level_bindings.get(base) == Some(&JitCaptureKind::Vec3)
        {
            return Some(JitCaptureKind::Scalar);
        }
        None
    }

    fn collect_expr(
        expr: &Expr,
        locals: &[String],
        captures: &mut Vec<JitCapture>,
        top_level_bindings: &HashMap<String, JitCaptureKind>,
        functions: &HashMap<String, (Vec<String>, Vec<MaterialFunctionStatement>)>,
        seen_functions: &mut Vec<String>,
    ) {
        match expr {
            Expr::Number(_) | Expr::String(_) => {}
            Expr::Array(items) => {
                for item in items {
                    collect_expr(
                        item,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::Ident(name) => {
                if !locals.iter().any(|local| local == name)
                    && let Some(kind) = known_capture_kind(name, top_level_bindings)
                    && !captures.iter().any(|capture| capture.name == *name)
                {
                    captures.push(JitCapture {
                        name: name.clone(),
                        kind,
                    });
                }
            }
            Expr::ObjectLiteral { fields, .. } => {
                for (_, value) in fields {
                    collect_expr(
                        value,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                collect_expr(
                    lhs,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
                collect_expr(
                    rhs,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
            }
            Expr::Member { target, .. } => {
                if let Some(name) = flatten_expr(expr)
                    && let Some(kind) = known_capture_kind(&name, top_level_bindings)
                {
                    if !captures.iter().any(|capture| capture.name == name) {
                        captures.push(JitCapture { name, kind });
                    }
                } else {
                    collect_expr(
                        target,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::Unary { expr, .. } => {
                collect_expr(
                    expr,
                    locals,
                    captures,
                    top_level_bindings,
                    functions,
                    seen_functions,
                );
            }
            Expr::Call { callee, args } => {
                if let Expr::Ident(name) = callee.as_ref()
                    && let Some((params, body)) = functions.get(name)
                    && !seen_functions.iter().any(|seen| seen == name)
                {
                    seen_functions.push(name.clone());
                    let mut fn_locals = locals.to_vec();
                    fn_locals.extend(params.iter().cloned());
                    for stmt in body {
                        match stmt {
                            MaterialFunctionStatement::Binding { name, expr } => {
                                collect_expr(
                                    expr,
                                    &fn_locals,
                                    captures,
                                    top_level_bindings,
                                    functions,
                                    seen_functions,
                                );
                                fn_locals.push(name.clone());
                            }
                            MaterialFunctionStatement::Return { expr } => {
                                collect_expr(
                                    expr,
                                    &fn_locals,
                                    captures,
                                    top_level_bindings,
                                    functions,
                                    seen_functions,
                                );
                            }
                        }
                    }
                    seen_functions.pop();
                }
                for arg in args {
                    collect_expr(
                        arg,
                        locals,
                        captures,
                        top_level_bindings,
                        functions,
                        seen_functions,
                    );
                }
            }
            Expr::FunctionLiteral { .. } => {}
        }
    }

    let mut captures = Vec::new();
    let mut locals = params.to_vec();
    let mut seen_functions = Vec::new();
    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                collect_expr(
                    expr,
                    &locals,
                    &mut captures,
                    top_level_bindings,
                    functions,
                    &mut seen_functions,
                );
                locals.push(name.clone());
            }
            MaterialFunctionStatement::Return { expr } => {
                collect_expr(
                    expr,
                    &locals,
                    &mut captures,
                    top_level_bindings,
                    functions,
                    &mut seen_functions,
                );
            }
        }
    }
    captures.sort_by(|a, b| a.name.cmp(&b.name));
    captures.dedup_by(|a, b| a.name == b.name);
    captures
}

fn flatten_expr(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::Member { target, field } => Some(format!("{}.{}", flatten_expr(target)?, field)),
        _ => None,
    }
}

fn compile_material_expr(
    expr: &Expr,
    ctx: &mut MaterialJitContext<'_, '_>,
) -> Option<MaterialJitValue> {
    match expr {
        Expr::Number(value) => Some(MaterialJitValue::Scalar(ctx.fb.ins().f64const(*value))),
        Expr::Ident(name) => {
            if let Some(value) = ctx.locals.get(name) {
                return Some(*value);
            }
            if let Some(value) = ctx.captures.get(name) {
                return Some(*value);
            }
            None
        }
        Expr::Member { target, field } => {
            if let Some(name) = flatten_expr(expr)
                && let Some(value) = ctx.captures.get(&name)
            {
                return Some(*value);
            }
            let value = compile_material_expr(target, ctx)?;
            let MaterialJitValue::Vec3(vec) = value else {
                return None;
            };
            match field.as_str() {
                "x" => Some(MaterialJitValue::Scalar(vec[0])),
                "y" => Some(MaterialJitValue::Scalar(vec[1])),
                "z" => Some(MaterialJitValue::Scalar(vec[2])),
                _ => None,
            }
        }
        Expr::ObjectLiteral { type_name, fields } if type_name == "vec3" => {
            let zero = ctx.fb.ins().f64const(0.0);
            let mut values = [zero, zero, zero];
            for (name, expr) in fields {
                let MaterialJitValue::Scalar(value) = compile_material_expr(expr, ctx)? else {
                    return None;
                };
                match name.as_str() {
                    "x" => values[0] = value,
                    "y" => values[1] = value,
                    "z" => values[2] = value,
                    _ => return None,
                }
            }
            Some(MaterialJitValue::Vec3(values))
        }
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
        } => match compile_material_expr(expr, ctx)? {
            MaterialJitValue::Scalar(value) => {
                Some(MaterialJitValue::Scalar(ctx.fb.ins().fneg(value)))
            }
            MaterialJitValue::Vec3(values) => Some(MaterialJitValue::Vec3([
                ctx.fb.ins().fneg(values[0]),
                ctx.fb.ins().fneg(values[1]),
                ctx.fb.ins().fneg(values[2]),
            ])),
        },
        Expr::Binary { lhs, op, rhs } => {
            let lhs = compile_material_expr(lhs, ctx)?;
            let rhs = compile_material_expr(rhs, ctx)?;
            compile_material_binary(*op, lhs, rhs, ctx)
        }
        Expr::Call { callee, args } => {
            if let Expr::Ident(name) = callee.as_ref() {
                if let Some((params, body)) = ctx.functions.get(name).cloned() {
                    let arg_values = args
                        .iter()
                        .map(|arg| compile_material_expr(arg, ctx))
                        .collect::<Option<Vec<_>>>()?;
                    return compile_inline_material_function(&params, &body, arg_values, ctx);
                }
                let arg_values = args
                    .iter()
                    .map(|arg| compile_material_expr(arg, ctx))
                    .collect::<Option<Vec<_>>>()?;
                return compile_material_builtin(name, &arg_values, ctx);
            }
            None
        }
        _ => None,
    }
}

fn compile_inline_material_function(
    params: &[String],
    body: &[MaterialFunctionStatement],
    args: Vec<MaterialJitValue>,
    ctx: &mut MaterialJitContext<'_, '_>,
) -> Option<MaterialJitValue> {
    let old_locals = ctx.locals.clone();
    for (param, value) in params.iter().zip(args.into_iter()) {
        ctx.locals.insert(param.clone(), value);
    }
    let mut result = None;
    for stmt in body {
        match stmt {
            MaterialFunctionStatement::Binding { name, expr } => {
                let value = compile_material_expr(expr, ctx)?;
                ctx.locals.insert(name.clone(), value);
            }
            MaterialFunctionStatement::Return { expr } => {
                result = Some(compile_material_expr(expr, ctx)?);
                break;
            }
        }
    }
    ctx.locals = old_locals;
    result
}

fn compile_material_binary(
    op: BinaryOp,
    lhs: MaterialJitValue,
    rhs: MaterialJitValue,
    ctx: &mut MaterialJitContext<'_, '_>,
) -> Option<MaterialJitValue> {
    let scalar = |ctx: &mut MaterialJitContext<'_, '_>,
                  lhs: cranelift_codegen::ir::Value,
                  rhs: cranelift_codegen::ir::Value|
     -> Option<cranelift_codegen::ir::Value> {
        Some(match op {
            BinaryOp::Add => ctx.fb.ins().fadd(lhs, rhs),
            BinaryOp::Sub => ctx.fb.ins().fsub(lhs, rhs),
            BinaryOp::Mul => ctx.fb.ins().fmul(lhs, rhs),
            BinaryOp::Div => ctx.fb.ins().fdiv(lhs, rhs),
            BinaryOp::Intersect => return None,
        })
    };
    match (lhs, rhs) {
        (MaterialJitValue::Scalar(lhs), MaterialJitValue::Scalar(rhs)) => {
            Some(MaterialJitValue::Scalar(scalar(ctx, lhs, rhs)?))
        }
        (MaterialJitValue::Vec3(lhs), MaterialJitValue::Vec3(rhs)) => {
            Some(MaterialJitValue::Vec3([
                scalar(ctx, lhs[0], rhs[0])?,
                scalar(ctx, lhs[1], rhs[1])?,
                scalar(ctx, lhs[2], rhs[2])?,
            ]))
        }
        (MaterialJitValue::Vec3(lhs), MaterialJitValue::Scalar(rhs)) => {
            Some(MaterialJitValue::Vec3([
                scalar(ctx, lhs[0], rhs)?,
                scalar(ctx, lhs[1], rhs)?,
                scalar(ctx, lhs[2], rhs)?,
            ]))
        }
        (MaterialJitValue::Scalar(lhs), MaterialJitValue::Vec3(rhs)) => {
            Some(MaterialJitValue::Vec3([
                scalar(ctx, lhs, rhs[0])?,
                scalar(ctx, lhs, rhs[1])?,
                scalar(ctx, lhs, rhs[2])?,
            ]))
        }
    }
}

fn compile_material_builtin(
    name: &str,
    args: &[MaterialJitValue],
    ctx: &mut MaterialJitContext<'_, '_>,
) -> Option<MaterialJitValue> {
    match (name, args) {
        ("vec3", [MaterialJitValue::Scalar(v)]) => Some(MaterialJitValue::Vec3([*v, *v, *v])),
        (
            "vec3",
            [
                MaterialJitValue::Scalar(x),
                MaterialJitValue::Scalar(y),
                MaterialJitValue::Scalar(z),
            ],
        ) => Some(MaterialJitValue::Vec3([*x, *y, *z])),
        ("abs", [MaterialJitValue::Scalar(v)]) => {
            Some(MaterialJitValue::Scalar(ctx.fb.ins().fabs(*v)))
        }
        ("abs", [MaterialJitValue::Vec3(values)]) => Some(MaterialJitValue::Vec3([
            ctx.fb.ins().fabs(values[0]),
            ctx.fb.ins().fabs(values[1]),
            ctx.fb.ins().fabs(values[2]),
        ])),
        ("min", [lhs, rhs]) => compile_material_min_max(lhs, rhs, ctx, false),
        ("max", [lhs, rhs]) => compile_material_min_max(lhs, rhs, ctx, true),
        ("length", [MaterialJitValue::Vec3(values)]) => {
            let x2 = ctx.fb.ins().fmul(values[0], values[0]);
            let y2 = ctx.fb.ins().fmul(values[1], values[1]);
            let z2 = ctx.fb.ins().fmul(values[2], values[2]);
            let sum_xy = ctx.fb.ins().fadd(x2, y2);
            let sum = ctx.fb.ins().fadd(sum_xy, z2);
            Some(MaterialJitValue::Scalar(ctx.fb.ins().sqrt(sum)))
        }
        ("normalize", [MaterialJitValue::Vec3(values)]) => {
            let x2 = ctx.fb.ins().fmul(values[0], values[0]);
            let y2 = ctx.fb.ins().fmul(values[1], values[1]);
            let z2 = ctx.fb.ins().fmul(values[2], values[2]);
            let sum_xy = ctx.fb.ins().fadd(x2, y2);
            let sum = ctx.fb.ins().fadd(sum_xy, z2);
            let len = ctx.fb.ins().sqrt(sum);
            let eps = ctx.fb.ins().f64const(1.0e-9);
            let safe_len = ctx.fb.ins().fmax(len, eps);
            Some(MaterialJitValue::Vec3([
                ctx.fb.ins().fdiv(values[0], safe_len),
                ctx.fb.ins().fdiv(values[1], safe_len),
                ctx.fb.ins().fdiv(values[2], safe_len),
            ]))
        }
        ("sin", [MaterialJitValue::Scalar(v)]) => {
            emit_unary_import_call(ctx.fb, ctx.module, "sin", *v).map(MaterialJitValue::Scalar)
        }
        ("cos", [MaterialJitValue::Scalar(v)]) => {
            emit_unary_import_call(ctx.fb, ctx.module, "cos", *v).map(MaterialJitValue::Scalar)
        }
        ("step", [MaterialJitValue::Scalar(edge), MaterialJitValue::Scalar(x)]) => {
            let cond = ctx.fb.ins().fcmp(
                cranelift_codegen::ir::condcodes::FloatCC::LessThan,
                *x,
                *edge,
            );
            let zero = ctx.fb.ins().f64const(0.0);
            let one = ctx.fb.ins().f64const(1.0);
            Some(MaterialJitValue::Scalar(
                ctx.fb.ins().select(cond, zero, one),
            ))
        }
        (
            "clamp",
            [
                MaterialJitValue::Scalar(x),
                MaterialJitValue::Scalar(a),
                MaterialJitValue::Scalar(b),
            ],
        ) => {
            let maxed = ctx.fb.ins().fmax(*x, *a);
            Some(MaterialJitValue::Scalar(ctx.fb.ins().fmin(maxed, *b)))
        }
        ("rotate_x", [MaterialJitValue::Vec3(v), MaterialJitValue::Scalar(deg)]) => Some(
            MaterialJitValue::Vec3(emit_material_rotate_vec3(ctx, *v, *deg, 0)?),
        ),
        ("rotate_y", [MaterialJitValue::Vec3(v), MaterialJitValue::Scalar(deg)]) => Some(
            MaterialJitValue::Vec3(emit_material_rotate_vec3(ctx, *v, *deg, 1)?),
        ),
        ("rotate_z", [MaterialJitValue::Vec3(v), MaterialJitValue::Scalar(deg)]) => Some(
            MaterialJitValue::Vec3(emit_material_rotate_vec3(ctx, *v, *deg, 2)?),
        ),
        ("mix", [lhs, rhs, MaterialJitValue::Scalar(a)]) => {
            let one = ctx.fb.ins().f64const(1.0);
            let inv = ctx.fb.ins().fsub(one, *a);
            match (lhs, rhs) {
                (MaterialJitValue::Scalar(x), MaterialJitValue::Scalar(y)) => {
                    let lhs = ctx.fb.ins().fmul(*x, inv);
                    let rhs = ctx.fb.ins().fmul(*y, *a);
                    Some(MaterialJitValue::Scalar(ctx.fb.ins().fadd(lhs, rhs)))
                }
                (MaterialJitValue::Vec3(x), MaterialJitValue::Vec3(y)) => {
                    let x0 = ctx.fb.ins().fmul(x[0], inv);
                    let y0 = ctx.fb.ins().fmul(y[0], *a);
                    let x1 = ctx.fb.ins().fmul(x[1], inv);
                    let y1 = ctx.fb.ins().fmul(y[1], *a);
                    let x2 = ctx.fb.ins().fmul(x[2], inv);
                    let y2 = ctx.fb.ins().fmul(y[2], *a);
                    Some(MaterialJitValue::Vec3([
                        ctx.fb.ins().fadd(x0, y0),
                        ctx.fb.ins().fadd(x1, y1),
                        ctx.fb.ins().fadd(x2, y2),
                    ]))
                }
                (MaterialJitValue::Vec3(x), MaterialJitValue::Scalar(y)) => {
                    let x0 = ctx.fb.ins().fmul(x[0], inv);
                    let y0 = ctx.fb.ins().fmul(*y, *a);
                    let x1 = ctx.fb.ins().fmul(x[1], inv);
                    let y1 = ctx.fb.ins().fmul(*y, *a);
                    let x2 = ctx.fb.ins().fmul(x[2], inv);
                    let y2 = ctx.fb.ins().fmul(*y, *a);
                    Some(MaterialJitValue::Vec3([
                        ctx.fb.ins().fadd(x0, y0),
                        ctx.fb.ins().fadd(x1, y1),
                        ctx.fb.ins().fadd(x2, y2),
                    ]))
                }
                (MaterialJitValue::Scalar(x), MaterialJitValue::Vec3(y)) => {
                    let x0 = ctx.fb.ins().fmul(*x, inv);
                    let y0 = ctx.fb.ins().fmul(y[0], *a);
                    let x1 = ctx.fb.ins().fmul(*x, inv);
                    let y1 = ctx.fb.ins().fmul(y[1], *a);
                    let x2 = ctx.fb.ins().fmul(*x, inv);
                    let y2 = ctx.fb.ins().fmul(y[2], *a);
                    Some(MaterialJitValue::Vec3([
                        ctx.fb.ins().fadd(x0, y0),
                        ctx.fb.ins().fadd(x1, y1),
                        ctx.fb.ins().fadd(x2, y2),
                    ]))
                }
            }
        }
        ("smoothstep", [edge0, edge1, x]) => {
            let (
                MaterialJitValue::Scalar(edge0),
                MaterialJitValue::Scalar(edge1),
                MaterialJitValue::Scalar(x),
            ) = (edge0, edge1, x)
            else {
                return None;
            };
            let span = ctx.fb.ins().fsub(*edge1, *edge0);
            let x_minus_edge0 = ctx.fb.ins().fsub(*x, *edge0);
            let t = ctx.fb.ins().fdiv(x_minus_edge0, span);
            let zero = ctx.fb.ins().f64const(0.0);
            let one = ctx.fb.ins().f64const(1.0);
            let t_max = ctx.fb.ins().fmax(t, zero);
            let t = ctx.fb.ins().fmin(t_max, one);
            let t2 = ctx.fb.ins().fmul(t, t);
            let three = ctx.fb.ins().f64const(3.0);
            let two = ctx.fb.ins().f64const(2.0);
            let two_t = ctx.fb.ins().fmul(two, t);
            let cubic = ctx.fb.ins().fsub(three, two_t);
            Some(MaterialJitValue::Scalar(ctx.fb.ins().fmul(t2, cubic)))
        }
        _ => None,
    }
}

fn emit_material_rotate_vec3(
    ctx: &mut MaterialJitContext<'_, '_>,
    v: [cranelift_codegen::ir::Value; 3],
    deg: cranelift_codegen::ir::Value,
    axis: u8,
) -> Option<[cranelift_codegen::ir::Value; 3]> {
    let pi_over_180 = ctx.fb.ins().f64const(std::f64::consts::PI / 180.0);
    let radians = ctx.fb.ins().fmul(deg, pi_over_180);
    let s = emit_unary_import_call(ctx.fb, ctx.module, "sin", radians)?;
    let c = emit_unary_import_call(ctx.fb, ctx.module, "cos", radians)?;
    match axis {
        0 => {
            let cy = ctx.fb.ins().fmul(c, v[1]);
            let sz = ctx.fb.ins().fmul(s, v[2]);
            let sy = ctx.fb.ins().fmul(s, v[1]);
            let cz = ctx.fb.ins().fmul(c, v[2]);
            Some([v[0], ctx.fb.ins().fsub(cy, sz), ctx.fb.ins().fadd(sy, cz)])
        }
        1 => {
            let cx = ctx.fb.ins().fmul(c, v[0]);
            let sz = ctx.fb.ins().fmul(s, v[2]);
            let sx = ctx.fb.ins().fmul(s, v[0]);
            let cz = ctx.fb.ins().fmul(c, v[2]);
            Some([ctx.fb.ins().fadd(cx, sz), v[1], ctx.fb.ins().fsub(cz, sx)])
        }
        _ => {
            let cx = ctx.fb.ins().fmul(c, v[0]);
            let sy = ctx.fb.ins().fmul(s, v[1]);
            let sx = ctx.fb.ins().fmul(s, v[0]);
            let cy = ctx.fb.ins().fmul(c, v[1]);
            Some([ctx.fb.ins().fsub(cx, sy), ctx.fb.ins().fadd(sx, cy), v[2]])
        }
    }
}

fn compile_material_min_max(
    lhs: &MaterialJitValue,
    rhs: &MaterialJitValue,
    ctx: &mut MaterialJitContext<'_, '_>,
    is_max: bool,
) -> Option<MaterialJitValue> {
    let scalar = |ctx: &mut MaterialJitContext<'_, '_>,
                  lhs: cranelift_codegen::ir::Value,
                  rhs: cranelift_codegen::ir::Value|
     -> cranelift_codegen::ir::Value {
        if is_max {
            ctx.fb.ins().fmax(lhs, rhs)
        } else {
            ctx.fb.ins().fmin(lhs, rhs)
        }
    };
    match (*lhs, *rhs) {
        (MaterialJitValue::Scalar(lhs), MaterialJitValue::Scalar(rhs)) => {
            Some(MaterialJitValue::Scalar(scalar(ctx, lhs, rhs)))
        }
        (MaterialJitValue::Vec3(lhs), MaterialJitValue::Vec3(rhs)) => {
            Some(MaterialJitValue::Vec3([
                scalar(ctx, lhs[0], rhs[0]),
                scalar(ctx, lhs[1], rhs[1]),
                scalar(ctx, lhs[2], rhs[2]),
            ]))
        }
        (MaterialJitValue::Vec3(lhs), MaterialJitValue::Scalar(rhs)) => {
            Some(MaterialJitValue::Vec3([
                scalar(ctx, lhs[0], rhs),
                scalar(ctx, lhs[1], rhs),
                scalar(ctx, lhs[2], rhs),
            ]))
        }
        (MaterialJitValue::Scalar(lhs), MaterialJitValue::Vec3(rhs)) => {
            Some(MaterialJitValue::Vec3([
                scalar(ctx, lhs, rhs[0]),
                scalar(ctx, lhs, rhs[1]),
                scalar(ctx, lhs, rhs[2]),
            ]))
        }
    }
}

fn emit_unary_import_call(
    fb: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    symbol: &str,
    arg: cranelift_codegen::ir::Value,
) -> Option<cranelift_codegen::ir::Value> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(types::F64));
    sig.returns.push(AbiParam::new(types::F64));
    let func_id = module
        .declare_function(symbol, Linkage::Import, &sig)
        .ok()?;
    let local = module.declare_func_in_func(func_id, fb.func);
    let call = fb.ins().call(local, &[arg]);
    let results = fb.inst_results(call);
    results.first().copied()
}

fn emit_builtin_call(
    fb: &mut FunctionBuilder<'_>,
    module: &mut JITModule,
    name: &str,
    args: &[cranelift_codegen::ir::Value],
) -> Option<cranelift_codegen::ir::Value> {
    match (name, args) {
        ("abs", [x]) => Some(fb.ins().fabs(*x)),
        ("min", [a, b]) => Some(fb.ins().fmin(*a, *b)),
        ("max", [a, b]) => Some(fb.ins().fmax(*a, *b)),
        ("sqrt", [x]) => Some(fb.ins().sqrt(*x)),
        ("floor", [x]) => Some(fb.ins().floor(*x)),
        ("ceil", [x]) => Some(fb.ins().ceil(*x)),
        ("sin", [x]) => emit_unary_import_call(fb, module, "sin", *x),
        ("cos", [x]) => emit_unary_import_call(fb, module, "cos", *x),
        ("saturate", [x]) => {
            let zero = fb.ins().f64const(0.0);
            let one = fb.ins().f64const(1.0);
            let maxed = fb.ins().fmax(*x, zero);
            Some(fb.ins().fmin(maxed, one))
        }
        ("clamp", [x, a, b]) => {
            let maxed = fb.ins().fmax(*x, *a);
            Some(fb.ins().fmin(maxed, *b))
        }
        ("mix", [x, y, a]) => {
            let one = fb.ins().f64const(1.0);
            let inv = fb.ins().fsub(one, *a);
            let lhs = fb.ins().fmul(*x, inv);
            let rhs = fb.ins().fmul(*y, *a);
            Some(fb.ins().fadd(lhs, rhs))
        }
        ("step", [edge, x]) => {
            let cond = fb.ins().fcmp(
                cranelift_codegen::ir::condcodes::FloatCC::LessThan,
                *x,
                *edge,
            );
            let zero = fb.ins().f64const(0.0);
            let one = fb.ins().f64const(1.0);
            Some(fb.ins().select(cond, zero, one))
        }
        ("smoothstep", [edge0, edge1, x]) => {
            let span = fb.ins().fsub(*edge1, *edge0);
            let x_minus_edge0 = fb.ins().fsub(*x, *edge0);
            let t = fb.ins().fdiv(x_minus_edge0, span);
            let zero = fb.ins().f64const(0.0);
            let one = fb.ins().f64const(1.0);
            let maxed = fb.ins().fmax(t, zero);
            let t = fb.ins().fmin(maxed, one);
            let three = fb.ins().f64const(3.0);
            let two = fb.ins().f64const(2.0);
            let two_t = fb.ins().fmul(two, t);
            let inner = fb.ins().fsub(three, two_t);
            Some(fb.ins().fmul(t, inner))
        }
        ("fract", [x]) => {
            let floor = fb.ins().floor(*x);
            Some(fb.ins().fsub(*x, floor))
        }
        _ => None,
    }
}
