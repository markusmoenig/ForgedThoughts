use crate::prelude::*;
use rhai::{Engine};


#[derive(PartialEq, Debug, Clone)]
pub struct Texture {
    pub buffer          : Option<ColorBuffer>,
}

impl Texture {

    pub fn new(width: i64, height: i64) -> Self {
        Self {
            buffer      : Some(ColorBuffer::new(width as usize, height as usize))
        }
    }

    pub fn empty() -> Self {
        Self {
            buffer      : None,
        }
    }

    pub fn pixel(&self, uv: F2, back: &Color) -> Color {

        pub fn mix_color(a: &[F], b: &[F], v: F) -> [F; 4] {
            [   (1.0 - v) * a[0] + b[0] * v,
                (1.0 - v) * a[1] + b[1] * v,
                (1.0 - v) * a[2] + b[2] * v,
                (1.0 - v) * a[3] + b[3] * v]
        }

        let d = uv.length() - 0.35;

        let mask = smoothstep(0.0, -0.002, d);

        let c = mix_color(back, &[1.0, 0.0, 0.0, 1.0], mask);

        c
    }

    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Texture>("Texture")
            .register_fn("Texture", Texture::new)
            .register_fn("Texture", Texture::empty);
            // .register_get_set("rgb", Light::get_rgb, Light::set_rgb)
            // .register_get_set("position", Light::get_position, Light::set_position)
            // .register_get_set("radius", Light::get_radius, Light::set_radius)
            // .register_get_set("intensity", Light::get_intensity, Light::set_intensity);
    }
}