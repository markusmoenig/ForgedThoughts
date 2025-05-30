pub mod camera;
pub mod ft;
pub mod lights;
pub mod marching_cubes;
pub mod material;
pub mod modelbuffer;
pub mod node;
pub mod ray;
pub mod renderbuffer;
pub mod renderer;
pub mod scanner;
pub mod utils;

#[cfg(feature = "double")]
pub type F = f64;

#[cfg(not(feature = "double"))]
pub type F = f32;

#[cfg(feature = "double")]
pub const F_PI: F = std::f64::consts::PI;
#[cfg(feature = "double")]
pub const F_TAU: F = std::f64::consts::TAU;
#[cfg(feature = "double")]
pub const F_FRAC_PI_2: F = std::f64::consts::FRAC_PI_2;
#[cfg(feature = "double")]
pub const F_FRAC_1_PI: F = std::f64::consts::FRAC_1_PI;
#[cfg(feature = "double")]
pub const F_E: F = std::f64::consts::E;
#[cfg(feature = "double")]
pub const F_SQRT_2: F = std::f64::consts::SQRT_2;
#[cfg(feature = "double")]
pub const F_MIN: F = std::f64::MIN;
#[cfg(feature = "double")]
pub const F_MAX: F = std::f64::MAX;

#[cfg(not(feature = "double"))]
pub const F_PI: F = std::f32::consts::PI;
#[cfg(not(feature = "double"))]
pub const F_TAU: F = std::f32::consts::TAU;
#[cfg(not(feature = "double"))]
pub const F_FRAC_PI_2: F = std::f32::consts::FRAC_PI_2;
#[cfg(not(feature = "double"))]
pub const F_FRAC_1_PI: F = std::f32::consts::FRAC_1_PI;
#[cfg(not(feature = "double"))]
pub const F_E: F = std::f32::consts::E;
#[cfg(not(feature = "double"))]
pub const F_SQRT_2: F = std::f32::consts::SQRT_2;
#[cfg(not(feature = "double"))]
pub const F_MIN: F = f32::MIN;
#[cfg(not(feature = "double"))]
pub const F_MAX: F = f32::MAX;

/// Abstraction for a single color value (either f32 or f64)
pub type Color = [F; 4];

/// Abstraction for a single voxel value (either f32 or f64)
#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    pub distance: F,
    pub density: F,
    pub material: u16,
}

#[inline(always)]
pub fn lerp(a: F, b: F, t: F) -> F {
    a + t * (b - a)
}

// Re-exports
pub use crate::{
    camera::{pinhole::Pinhole, Camera},
    ft::FT,
    lights::{point::PointLight, Light},
    marching_cubes::MarchingCubes,
    material::Material,
    modelbuffer::ModelBuffer,
    node::{
        graph::Graph, terminal::NodeTerminal, terminal::NodeTerminalRole, Node, NodeDomain,
        NodeRole,
    },
    ray::{Hit, Ray},
    renderbuffer::RenderBuffer,
    renderer::{pbr::PBR, Renderer},
    scanner::{Scanner, TokenType},
};

pub mod prelude {
    pub use crate::lerp;
    pub use crate::FT;
    pub use crate::{Camera, Pinhole};
    pub use crate::{Color, Voxel, F};
    pub use crate::{Graph, Node, NodeDomain, NodeRole, NodeTerminal, NodeTerminalRole};
    pub use crate::{Hit, Material, Ray};
    pub use crate::{Light, PointLight};
    pub use crate::{ModelBuffer, RenderBuffer};
    pub use crate::{Renderer, PBR};
}
