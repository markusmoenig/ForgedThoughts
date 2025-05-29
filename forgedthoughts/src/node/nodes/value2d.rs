use crate::prelude::*;
use vek::{Mat2, Vec2, Vec3, Vec4};

pub struct ValueNoise2D {}

impl Node for ValueNoise2D {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn name(&self) -> &str {
        "ValueNoise2D"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Node
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D2
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("scale", NodeTerminalRole::Vec2(Vec2::new(1.0, 1.0)), ""),
            NodeTerminal::new("octaves", NodeTerminalRole::Vec1(3.0), ""),
            NodeTerminal::new("offset", NodeTerminalRole::Vec2(Vec2::zero()), ""),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec4(Vec4::broadcast(0.0)),
            "x",
        )]
    }

    fn evaluate_2d(&self, uv: Vec2<F>, _resolution: Vec2<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        let octaves = inputs[1].x as i32;

        fn hash(p: Vec2<f32>) -> f32 {
            let mut p3 = Vec3::new(p.x, p.y, p.x).map(|v| (v * 0.13).fract());
            p3 += p3.dot(Vec3::new(p3.y, p3.z, p3.x) + 3.333);
            ((p3.x + p3.y) * p3.z).fract()
        }

        fn noise(x: Vec2<f32>) -> f32 {
            let i = x.map(|v| v.floor());
            let f = x.map(|v| v.fract());

            let a = hash(i);
            let b = hash(i + Vec2::new(1.0, 0.0));
            let c = hash(i + Vec2::new(0.0, 1.0));
            let d = hash(i + Vec2::new(1.0, 1.0));

            let u = f * f * f.map(|v| 3.0 - 2.0 * v);
            lerp(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y
        }

        let mut x = uv * 20.0 / inputs[0].xy() + inputs[2].xy();

        if octaves == 0 {
            return Vec4::broadcast(noise(x));
        }

        let mut v = 0.0;
        let mut a = 0.5;
        let shift = Vec2::new(100.0, 100.0);
        let rot = Mat2::new(0.5f32.cos(), 0.5f32.sin(), -0.5f32.sin(), 0.5f32.cos());
        for _ in 0..octaves {
            v += a * noise(x);
            x = rot * x * 2.0 + shift;
            a *= 0.5;
        }
        Vec4::broadcast(v)
    }
}
