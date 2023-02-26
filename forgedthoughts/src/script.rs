
use crate::prelude::*;

pub use rhai::{Engine, AST, Scope};

/// FTContext
pub struct FTContext<'a> {
    pub engine      : Engine,
    pub ast         : AST,
    pub scope       : Scope<'a>,
    pub settings    : Settings,
    pub camera      : Camera,
    pub scene       : Scene,
}

/// Create an Rhai engine instance and register all FT types
pub fn create_engine() -> Engine {
    let mut engine = Engine::new();

    engine.set_fast_operators(false);

    // Vectors
    F2::register(&mut engine);
    F3::register(&mut engine);
    B3::register(&mut engine);

    // -- Renderer
    Renderer::register(&mut engine);

    // -- Settings
    Settings::register(&mut engine);

    // -- Camera
    Camera::register(&mut engine);

    // -- Material
    Material::register(&mut engine);

    // -- HitRecord
    HitRecord::register(&mut engine);

    // -- Lights
    Light::register(&mut engine);

    // -- SDF Types
    SDF::register(&mut engine);

    // --Modifer
    RayModifier::register(&mut engine);

    // -- Math functions
    crate::ft::math::register_math(&mut engine);


    engine.on_print(|x| println!("{}", x));

    engine
}