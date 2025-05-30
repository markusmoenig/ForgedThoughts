use crate::prelude::*;
use vek::{Vec2, Vec3, Vec4};

pub struct MaterialNode {}

impl Node for MaterialNode {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "Material"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Material
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D2
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("albedo", NodeTerminalRole::Vec3(Vec3::one()), ""),
            NodeTerminal::new("emission", NodeTerminalRole::Vec3(Vec3::zero()), ""),
            NodeTerminal::new("roughness", NodeTerminalRole::Vec1(0.5), ""),
            NodeTerminal::new("metallic", NodeTerminalRole::Vec1(0.0), ""),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec1(0.0),
            "x",
        )]
    }

    fn evaluate_2d(&self, _uv: Vec2<F>, _resolution: Vec2<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        inputs[0]
    }
}
