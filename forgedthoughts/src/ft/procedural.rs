use crate::prelude::*;

use rhai::{Engine};

pub fn checker(p: F2, size: F) -> F {
    let a = (size * p.x).floor();
    let b = (size * p.y).floor();

    ((a + b) % 2.0).abs()
}

pub fn checker_xy(p: F2, sa: F, sb: F) -> F {
    let a = (sa * p.x).floor();
    let b = (sb * p.y).floor();

    ((a + b) % 2.0).abs()
}

fn to_linear(c: F3) -> F3 {
    F3::new(c.x.powf(2.2), c.y.powf(2.2), c.z.powf(2.2))
}

/// Register to the engine
pub fn register_procedurals(engine: &mut Engine) {
    engine.register_fn("checker", checker);
    engine.register_fn("checker", checker_xy);
    engine.register_fn("to_linear", to_linear);
}