
use crate::prelude::*;

pub use rhai::{Engine, AST, Scope};

/// FTContext
pub struct FTContext<'a> {
    pub engine      : Engine,
    pub ast         : AST,
    pub scope       : Scope<'a>,
    pub settings    : Settings,
}

/// Create an Rhai engine instance and register all FT types
pub fn create_engine() -> Engine {
    let mut engine = Engine::new();

    engine.set_fast_operators(false);

    engine.register_type_with_name::<F2>("F2")
        .register_fn("F2", F2::zeros)
        .register_fn("F2", F2::new)
        .register_fn("F2", F3::from)
        .register_fn("normalize", F2::normalize)
        .register_fn("length", F2::length)
        .register_fn("copy", F2::clone)
        .register_get_set("x", F2::get_x, F2::set_x)
        .register_get_set("y", F2::get_y, F2::set_y);

    engine.register_fn("+", |a: F2, b: F2| -> F2 {
        F2::new(a.x + b.x, a.y + b.y)
    });

    engine.register_fn("-", |a: F2, b: F2| -> F2 {
        F2::new(a.x - b.x, a.y - b.y)
    });

    // -- F3

    engine.register_type_with_name::<F3>("F3")
        .register_fn("F3", F3::zeros)
        .register_fn("F3", F3::new)
        .register_fn("F3", F3::from)
        .register_fn("normalize", F3::normalize)
        .register_fn("length", F3::length)
        .register_fn("copy", F3::clone)
        .register_get_set("x", F3::get_x, F3::set_x)
        .register_get_set("y", F3::get_y, F3::set_y)
        .register_get_set("z", F3::get_z, F3::set_z);

    engine.register_fn("+", |a: F3, b: F3| -> F3 {
        F3::new(a.x + b.x, a.y + b.y, a.z + b.z)
    });

    engine.register_fn("-", |a: F3, b: F3| -> F3 {
        F3::new(a.x - b.x, a.y - b.y, a.z - b.z)
    });

    // -- Settings

    engine.register_type_with_name::<Settings>("Settings")
        .register_fn("Settings", Settings::new)
        .register_get_set("width", Settings::get_width, Settings::set_width)
        .register_get_set("height", Settings::get_height, Settings::set_height)
        .register_get_set("antialias", Settings::get_antialias, Settings::set_antialias)
        .register_get_set("background", Settings::get_background, Settings::set_background);

    // -- Material

    engine.register_type_with_name::<Material>("Material")
        .register_fn("Material", Material::new)
        .register_get_set("rgb", Material::get_rgb, Material::set_rgb);

    // -- Light Types

    engine.register_type_with_name::<Light>("PointLight")
        .register_fn("PointLight", Light::new_point_light)
        .register_get_set("rgb", Light::get_rgb, Light::set_rgb)
        .register_get_set("position", Light::get_position, Light::set_position)
        .register_get_set("radius", Light::get_radius, Light::set_radius)
        .register_get_set("intensity", Light::get_intensity, Light::set_intensity);


    // -- SDF Types

    engine.register_type_with_name::<SDF>("Sphere")
        .register_fn("Sphere", SDF::new_sphere)
        .register_get_set("material", SDF::get_material, SDF::set_material)
        .register_get_set("position", SDF::get_position, SDF::set_position)
        .register_get_set("radius", SDF::get_radius, SDF::set_radius);


    engine.on_print(|x| println!("{}", x));

    engine
}