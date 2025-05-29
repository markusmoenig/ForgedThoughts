use crate::prelude::*;
use vek::{Vec3, Vec4};

pub struct ValueNoise3D;

impl Node for ValueNoise3D {
    fn new() -> Self {
        Self {}
    }

    fn name(&self) -> &str {
        "ValueNoise3D"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Node
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D3
    }

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new("scale", NodeTerminalRole::Vec3(Vec3::broadcast(1.0)), ""),
            NodeTerminal::new("octaves", NodeTerminalRole::Vec1(3.0), ""),
            NodeTerminal::new("offset", NodeTerminalRole::Vec3(Vec3::zero()), ""),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec4(Vec4::zero()),
            "x",
        )]
    }

    fn evaluate_3d(&self, pos: Vec3<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        fn hash(n: F) -> F {
            let mut n = n.fract() * 0.011;
            n *= n + 7.5;
            n *= n + n;
            n.fract()
        }

        fn noise(x: Vec3<F>) -> F {
            let step = Vec3::new(110.0, 241.0, 171.0);
            let i = x.map(|v| v.floor());
            let f = x.map(|v| v.fract());
            let n = i.dot(step);

            let u = f * f * (Vec3::broadcast(3.0) - 2.0 * f);

            let lerp = |a, b, t| a * (1.0 - t) + b * t;

            lerp(
                lerp(
                    lerp(
                        hash(n + step.dot(Vec3::new(0.0, 0.0, 0.0))),
                        hash(n + step.dot(Vec3::new(1.0, 0.0, 0.0))),
                        u.x,
                    ),
                    lerp(
                        hash(n + step.dot(Vec3::new(0.0, 1.0, 0.0))),
                        hash(n + step.dot(Vec3::new(1.0, 1.0, 0.0))),
                        u.x,
                    ),
                    u.y,
                ),
                lerp(
                    lerp(
                        hash(n + step.dot(Vec3::new(0.0, 0.0, 1.0))),
                        hash(n + step.dot(Vec3::new(1.0, 0.0, 1.0))),
                        u.x,
                    ),
                    lerp(
                        hash(n + step.dot(Vec3::new(0.0, 1.0, 1.0))),
                        hash(n + step.dot(Vec3::new(1.0, 1.0, 1.0))),
                        u.x,
                    ),
                    u.y,
                ),
                u.z,
            )
        }

        let scale = inputs[0].xyz();
        let octaves = inputs[1].x as i32;
        let offset = inputs[2].xyz();

        let mut x = pos / scale + offset;

        if octaves <= 0 {
            return Vec4::broadcast(noise(x));
        }

        let mut v = 0.0;
        let mut a = 0.5;
        let shift = Vec3::broadcast(100.0);
        for _ in 0..octaves {
            v += a * noise(x);
            x = x * 2.0 + shift;
            a *= 0.5;
        }

        Vec4::broadcast(v)
    }
}
