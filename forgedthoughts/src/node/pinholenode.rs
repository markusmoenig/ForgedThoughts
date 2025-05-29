use crate::prelude::*;
use vek::{Vec2, Vec3, Vec4};

pub struct PinholeNode {}

impl Node for PinholeNode {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "Pinhole"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Camera
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D2
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new(
                "origin",
                NodeTerminalRole::Vec3(Vec3::new(0.0, 1.0, 3.0)),
                "",
            ),
            NodeTerminal::new("center", NodeTerminalRole::Vec3(Vec3::zero()), ""),
            NodeTerminal::new("fov", NodeTerminalRole::Vec1(70.0), ""),
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
