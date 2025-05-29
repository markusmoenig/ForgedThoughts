use crate::prelude::*;
use std::sync::Arc;
use vek::{Vec2, Vec4};

pub struct PBR {}
use crate::camera::Camera;

use rand::Rng;

impl Renderer for PBR {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "PBR"
    }

    /// Render the pixel at the given screen position.
    fn render(
        &self,
        uv: Vec2<F>,
        resolution: Vec2<F>,
        ft: Arc<FT>,
        model: Arc<ModelBuffer>,
    ) -> Vec4<F> {
        let mut rng = rand::rng();
        let camera = Camera::default();
        let ray = camera.create_ray(uv, Vec2::new(rng.random(), rng.random()), resolution);

        if let Some(hit) = model.raymarch(&ray) {
            // println!("{}", hit.voxel.material);
            let material = ft.graph.evaluate_material(hit.voxel.material as usize, hit);

            Vec4::new(material[0].x, material[0].y, material[0].z, 1.0)
        } else {
            Vec4::zero()
        }
    }
}
