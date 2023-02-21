pub mod ft;
pub mod buffer;
pub mod script;

pub type I = i64;
pub type F = f64;

pub type Color = [F; 4];

pub mod prelude {

    pub use crate::I;
    pub use crate::F;
    pub use crate::Color;

    pub use crate::buffer::ColorBuffer;

    pub use crate::ft::FT;

    pub use crate::ft::fx::F2;
    pub use crate::ft::fx::F3;

    pub use crate::ft::sdf::SDF;
    pub use crate::ft::material::Material;
    pub use crate::ft::settings::Settings;

    pub use crate::script::FTContext;

}
