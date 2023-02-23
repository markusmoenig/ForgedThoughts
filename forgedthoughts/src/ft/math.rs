use crate::prelude::*;

use rhai::{Engine};

pub fn register_math(engine: &mut Engine) {

    engine.register_fn("mod", |a: F, b: F| -> F {
        a % b
    });

}
