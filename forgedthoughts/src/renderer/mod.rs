pub mod pbr;

use crate::prelude::*;
use std::sync::Arc;
use vek::{Vec2, Vec4};

#[allow(unused)]
pub trait Renderer: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Returns the name of the renderer.
    fn name(&self) -> &str;

    /// Render the pixel at the given screen position.
    fn render(
        &self,
        uv: Vec2<F>,
        resolution: Vec2<F>,
        ft: Arc<FT>,
        model: Arc<ModelBuffer>,
    ) -> Vec4<F> {
        Vec4::zero()
    }
}
