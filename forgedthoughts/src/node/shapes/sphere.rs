use crate::prelude::*;
use vek::{Vec3, Vec4};

pub struct Sphere {}

impl Node for Sphere {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "Sphere"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Shape
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D3
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("center", NodeTerminalRole::Vec3(Vec3::broadcast(0.0)), ""),
            NodeTerminal::new("radius", NodeTerminalRole::Vec1(1.0), ""),
            NodeTerminal::new("modifier", NodeTerminalRole::Vec1(0.0), ""),
            NodeTerminal::new("material", NodeTerminalRole::Vec1(1.0), ""),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec4(Vec4::broadcast(0.0)),
            "x",
        )]
    }

    fn evaluate_3d(&self, pos: Vec3<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        Vec4::broadcast((pos - inputs[0]).magnitude() - inputs[1].x) - inputs[2].x * 0.5
    }
}
