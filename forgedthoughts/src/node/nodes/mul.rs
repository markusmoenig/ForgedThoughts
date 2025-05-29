use crate::prelude::*;
use vek::{Vec2, Vec4};

pub struct Mul {}

impl Node for Mul {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "Mul"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Node
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D2
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("input", NodeTerminalRole::Vec4(Vec4::one()), ""),
            NodeTerminal::new("value", NodeTerminalRole::Vec4(Vec4::one()), ""),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec4(Vec4::zero()),
            "xyzw",
        )]
    }

    fn evaluate_2d(&self, _uv: Vec2<F>, _resolution: Vec2<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        inputs[0] * inputs[1]
    }
}
