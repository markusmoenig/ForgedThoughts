pub use crate::prelude::*;
use rayon::prelude::*;
use std::sync::Arc;
pub use vek::{Aabb, Vec3};

pub struct ModelBuffer {
    pub size: [usize; 3], // voxel resolution in x/y/z
    pub density: usize,   // voxels per unit
    pub bounds: [F; 3],   // world space bounds in units
    pub data: Vec<Voxel>, // flat voxel array
}

impl ModelBuffer {
    pub fn new(bounds: [F; 3], density: usize) -> Self {
        let size = [
            (bounds[0] * density as F).ceil() as usize,
            (bounds[1] * density as F).ceil() as usize,
            (bounds[2] * density as F).ceil() as usize,
        ];
        let total_voxels = size[0] * size[1] * size[2];

        ModelBuffer {
            size,
            density,
            bounds,
            data: vec![
                Voxel {
                    distance: F::MAX,
                    density: 0.0,
                    material: 0
                };
                total_voxels
            ],
        }
    }

    #[inline]
    pub fn index(&self, x: usize, y: usize, z: usize) -> usize {
        z * self.size[1] * self.size[0] + y * self.size[0] + x
    }

    #[inline]
    pub fn get_mut(&mut self, x: usize, y: usize, z: usize) -> Option<&mut Voxel> {
        let i = self.index(x, y, z);
        self.data.get_mut(i)
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Option<&Voxel> {
        let i = self.index(x, y, z);
        self.data.get(i)
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, voxel: Voxel) {
        let i = self.index(x, y, z);
        if i < self.data.len() {
            self.data[i] = voxel;
        }
    }

    #[inline]
    pub fn index_to_world(&self, x: usize, y: usize, z: usize) -> Vec3<F> {
        let size_f = Vec3::new(self.size[0] as F, self.size[1] as F, self.size[2] as F);
        let offset = Vec3::new(x as F, y as F, z as F);
        let voxel_size = Vec3::new(
            self.bounds[0] / size_f.x,
            self.bounds[1] / size_f.y,
            self.bounds[2] / size_f.z,
        );

        let pos = offset * voxel_size;

        Vec3::new(
            pos.x - self.bounds[0] / 2.0, // center X
            pos.y,                        // bottom-align Y
            pos.z - self.bounds[2] / 2.0, // center Z
        )
    }

    #[inline]
    pub fn world_to_index(&self, pos: Vec3<F>) -> Option<Vec3<usize>> {
        // Shift XZ to positive grid coordinates, Y is already bottom-aligned
        let shifted = Vec3::new(
            pos.x + self.bounds[0] / 2.0,
            pos.y,
            pos.z + self.bounds[2] / 2.0,
        );

        let scale = Vec3::new(
            self.size[0] as F / self.bounds[0],
            self.size[1] as F / self.bounds[1],
            self.size[2] as F / self.bounds[2],
        );

        let grid = shifted * scale;

        let x = grid.x.floor() as isize;
        let y = grid.y.floor() as isize;
        let z = grid.z.floor() as isize;

        if x >= 0
            && y >= 0
            && z >= 0
            && x < self.size[0] as isize
            && y < self.size[1] as isize
            && z < self.size[2] as isize
        {
            Some(Vec3::new(x as usize, y as usize, z as usize))
        } else {
            None
        }
    }

    /// Model the graph into the buffer.
    pub fn model(&mut self, ft: Arc<FT>) {
        let _start = ft.get_time();

        let size_x = self.size[0];
        let size_y = self.size[1];
        let size_z = self.size[2];

        let bounds = self.bounds;
        let data = &mut self.data;

        // pre-compute voxel size outside the loop
        let size_f = Vec3::new(size_x as F, size_y as F, size_z as F);
        let voxel_size = Vec3::new(
            bounds[0] / size_f.x,
            bounds[1] / size_f.y,
            bounds[2] / size_f.z,
        );
        let half_xz = Vec3::new(bounds[0] / 2.0, 0.0, bounds[2] / 2.0);

        // Create mutable z-slices: each (size_x * size_y)
        let z_slices: Vec<_> = data.chunks_mut(size_x * size_y).collect();

        z_slices.into_par_iter().enumerate().for_each(|(z, slice)| {
            for y in 0..size_y {
                for x in 0..size_x {
                    let i = y * size_x + x;

                    // *** bottom-aligned Y, centred XZ ***
                    let world = Vec3::new(
                        x as F * voxel_size.x - half_xz.x,
                        y as F * voxel_size.y, // 0 … bounds.y
                        z as F * voxel_size.z - half_xz.z,
                    );

                    let (distance, material) = ft.graph.evaluate_shapes(world);

                    if distance < slice[i].distance {
                        slice[i].distance = distance;
                        slice[i].material = material;
                    }
                }
            }
        });

        let _stop = ft.get_time();
        println!("Model execution time: {:?} ms.", _stop - _start);
    }

    /// Computes the normal at the given world position.
    pub fn compute_normal(&self, pos: Vec3<F>) -> Vec3<F> {
        // Estimate voxel size in world units
        let voxel_size = Vec3::new(
            self.bounds[0] / self.size[0] as F,
            self.bounds[1] / self.size[1] as F,
            self.bounds[2] / self.size[2] as F,
        );

        // Use average voxel size
        let eps = (voxel_size.x + voxel_size.y + voxel_size.z) / F::from(3.0);

        let dx = self.sample(pos + Vec3::new(eps, 0.0, 0.0))
            - self.sample(pos - Vec3::new(eps, 0.0, 0.0));
        let dy = self.sample(pos + Vec3::new(0.0, eps, 0.0))
            - self.sample(pos - Vec3::new(0.0, eps, 0.0));
        let dz = self.sample(pos + Vec3::new(0.0, 0.0, eps))
            - self.sample(pos - Vec3::new(0.0, 0.0, eps));

        Vec3::new(dx, dy, dz).normalized()
    }

    /// Samples the buffer at the given world position.
    pub fn sample(&self, pos: Vec3<F>) -> F {
        if let Some(index) = self.world_to_index(pos) {
            self.data[self.index(index.x, index.y, index.z)].distance
        } else {
            F::MAX
        }
    }

    /// Returns the bbox of the buffer centered at the origin.
    pub fn bbox(&self) -> Aabb<F> {
        Aabb {
            min: Vec3::new(-self.bounds[0] / 2.0, 0.0, -self.bounds[2] / 2.0),
            max: Vec3::new(self.bounds[0] / 2.0, self.bounds[1], self.bounds[2] / 2.0),
        }
    }

    /// Returns the amount of memory used as a String.
    pub fn memory_usage(&self) -> String {
        let bytes_per_voxel = std::mem::size_of::<Voxel>();
        let total_bytes = self.data.len() * bytes_per_voxel;

        if total_bytes >= 1024 * 1024 * 1024 {
            format!("{:.2} GB", total_bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        } else if total_bytes >= 1024 * 1024 {
            format!("{:.2} MB", total_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} KB", total_bytes as f64 / 1024.0)
        }
    }

    pub fn raymarch(&self, ray: &Ray) -> Option<Hit> {
        let bbox = self.bbox();

        let (t_min, t_max) = ray.intersect_aabb(&bbox)?;

        let epsilon = 0.001;
        let mut t = t_min.max(0.0) + epsilon;
        let max_distance = t_max.min(1000.0);
        let max_steps = 512;

        for _ in 0..max_steps {
            let p = ray.at(&t);
            let d = self.sample(p);

            if d < epsilon {
                // Hit — convert world pos to voxel and return Voxel
                let pos = self.world_to_index(p)?;
                let i = self.index(pos.x, pos.y, pos.z);
                let voxel = self.data[i];
                let normal = self.compute_normal(p);

                return Some(Hit {
                    position: p,
                    normal,
                    voxel,
                });
            }

            t += d * 0.5;

            if t > max_distance {
                break;
            }
        }

        None
    }
}
