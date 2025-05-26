//use crate::prelude::*;
use rhai::{Engine};


#[derive(PartialEq, Debug, Clone)]
pub struct Shapes {
}

impl Shapes {

    pub fn new() -> Self {
        Self {
        }
    }

    pub fn register(engine: &mut Engine) {
        engine.register_type_with_name::<Shapes>("Shapes")
            .register_fn("Shapes", Shapes::new);
    }
}