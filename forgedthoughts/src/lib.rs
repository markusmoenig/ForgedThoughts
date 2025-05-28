pub mod camera;
pub mod ft;
pub mod marching_cubes;
pub mod modelbuffer;
pub mod node;
pub mod ray;
pub mod renderbuffer;
pub mod scanner;
pub mod utils;

#[cfg(feature = "double")]
pub type F = f64;

#[cfg(not(feature = "double"))]
pub type F = f32;

/// Abstraction for a single color value (either f32 or f64)
pub type Color = [F; 4];

/// Abstraction for a single voxel value (either f32 or f64)
#[derive(Clone, Copy, Debug)]
pub struct Voxel {
    pub distance: F,
    pub density: F,
    pub material: u16,
}

use rust_embed::RustEmbed;
#[derive(RustEmbed)]
#[folder = "../nodes/"]
#[exclude = "*.txt"]
#[exclude = "*.DS_Store"]
pub struct Embedded;

// Re-exports
pub use crate::{
    ft::FT,
    marching_cubes::MarchingCubes,
    modelbuffer::ModelBuffer,
    node::{
        graph::Graph, terminal::NodeTerminal, terminal::NodeTerminalRole, Node, NodeExecutionCtx,
    },
    ray::{Hit, Ray},
    renderbuffer::RenderBuffer,
    scanner::{Scanner, TokenType},
};

pub mod prelude {
    pub use crate::FT;
    pub use crate::{Color, Voxel, F};
    pub use crate::{Graph, Node, NodeExecutionCtx, NodeTerminal, NodeTerminalRole};
    pub use crate::{Hit, Ray};
    pub use crate::{ModelBuffer, RenderBuffer};
}
