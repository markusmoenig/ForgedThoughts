pub mod ft;
pub mod buffer;

pub type I = i64;
pub type F = f64;

pub mod prelude {

    pub use crate::I;
    pub use crate::F;

    pub use crate::buffer::ColorBuffer;

    pub use crate::ft::FT;

    pub use crate::ft::fx::F2;
    pub use crate::ft::fx::F3;

    pub use crate::ft::sdf3d::SdfSphere3D;

}
