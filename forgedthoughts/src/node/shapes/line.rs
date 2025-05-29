use crate::prelude::*;
use vek::{Vec3, Vec4};

pub struct Line;

impl Node for Line {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn name(&self) -> &str {
        "Line"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Shape
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D3
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("pointA", NodeTerminalRole::Vec3(Vec3::zero()), ""),
            NodeTerminal::new("pointB", NodeTerminalRole::Vec3(Vec3::unit_x()), ""),
            NodeTerminal::new("radius", NodeTerminalRole::Vec1(0.1), ""),
            NodeTerminal::new("modifier", NodeTerminalRole::Vec1(0.0), ""),
            NodeTerminal::new("material", NodeTerminalRole::Vec1(0.0), ""),
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
        let a = inputs[0].xyz();
        let b = inputs[1].xyz();
        let r = inputs[2].x;
        let modifier = inputs[3].x;

        let pa = pos - a;
        let ba = b - a;
        let h = pa.dot(ba) / ba.dot(ba);
        let h_clamped = h.clamp(0.0, 1.0);
        let closest = ba * h_clamped;
        let distance = (pa - closest).magnitude() - r;

        Vec4::broadcast(distance - modifier * 0.5)
    }
}
