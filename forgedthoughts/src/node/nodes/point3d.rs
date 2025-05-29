use crate::prelude::*;
use vek::{Vec2, Vec3, Vec4};

pub struct Point3D {}

impl Node for Point3D {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "Point3D"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Node
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D2
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "position",
            NodeTerminalRole::Vec3(Vec3::one()),
            "",
        )]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "position",
            NodeTerminalRole::Vec3(Vec3::zero()),
            "xyz",
        )]
    }

    fn evaluate_2d(&self, _uv: Vec2<F>, _resolution: Vec2<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        inputs[0]
    }
}
