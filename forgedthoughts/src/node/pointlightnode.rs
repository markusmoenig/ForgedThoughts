use crate::prelude::*;
use vek::{Vec3, Vec4};

pub struct PointLightNode {}

impl Node for PointLightNode {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "PointLight"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Light
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D3
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("position", NodeTerminalRole::Vec3(Vec3::broadcast(0.5)), ""),
            NodeTerminal::new("color", NodeTerminalRole::Vec3(Vec3::broadcast(1.0)), ""),
            NodeTerminal::new("radius", NodeTerminalRole::Vec1(1.0), ""),
            NodeTerminal::new("intensity", NodeTerminalRole::Vec1(1.0), ""),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec4(Vec4::broadcast(0.0)),
            "x",
        )]
    }
}
