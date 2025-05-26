pub mod ft;
pub mod marching_cubes;
pub mod scanner;

pub type Color = [f64; 4];
pub type F = f64;

use rust_embed::RustEmbed;
#[derive(RustEmbed)]
#[folder = "../nodes/"]
#[exclude = "*.txt"]
#[exclude = "*.DS_Store"]
pub struct Embedded;

// Re-exports
pub use crate::{ft::FT, marching_cubes::MarchingCubes};

pub mod prelude {
    pub use crate::FT;
}
