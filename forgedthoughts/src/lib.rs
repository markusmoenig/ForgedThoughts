pub mod ft;
pub mod script;
pub mod renderer;
pub mod marching_cubes;

pub type Color = [f64; 4];
pub type F = f64;

pub mod prelude {

    pub use rust_pathtracer::prelude::*;

    pub use crate::Color;

    pub use rust_pathtracer::buffer::ColorBuffer;

    pub use crate::ft::FT;
    pub use crate::ft::analytical::Analytical;
    pub use crate::ft::sdf::SDF;
    pub use crate::ft::settings::Settings;
    pub use crate::ft::lights::Light;
    pub use crate::ft::camera::Camera;
    pub use crate::ft::scene::Scene;

    pub use crate::ft::renderer::RendererType;
    pub use crate::ft::renderer::Renderer;

    pub use crate::ft::structs::HitRecord;
    pub use crate::ft::ray_modifier::RayModifier;

    pub use crate::ft::operators::Smooth;
    pub use crate::ft::operators::Groove;

    pub use crate::renderer::phong::phong;
    pub use crate::renderer::pbr::pbr;

    pub use crate::renderer::bsdf::BSDFScene;
    pub use crate::renderer::bsdf::FTScene;

    pub use crate::script::FTContext;

    pub use crate::marching_cubes::MarchingCubes;

    pub use std::f64::consts::PI;

    pub use uuid::Uuid;
}
