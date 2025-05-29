use crate::prelude::*;
use vek::{Mat3, Vec3, Vec4};

/* ------------------------------------------------------------ */

pub struct Box;

impl Node for Box {
    fn new() -> Self {
        Self {}
    }

    fn name(&self) -> &str {
        "Box"
    }

    fn role(&self) -> NodeRole {
        NodeRole::Shape
    }

    fn domain(&self) -> NodeDomain {
        NodeDomain::D3
    }

    /* ------------------------- inputs ------------------------ */

    fn inputs(&self) -> Vec<NodeTerminal> {
        vec![
            NodeTerminal::new(
                "pointA",
                NodeTerminalRole::Vec3(Vec3::zero()),
                "start point",
            ),
            NodeTerminal::new(
                "pointB",
                NodeTerminalRole::Vec3(Vec3::new(0.0, 1.0, 0.0)),
                "end point",
            ),
            NodeTerminal::new(
                "size",
                NodeTerminalRole::Vec3(Vec3::broadcast(0.1)),
                "half-extents (X,Y)",
            ),
            NodeTerminal::new("modifier", NodeTerminalRole::Vec1(0.0), "distance modifier"),
            NodeTerminal::new("material", NodeTerminalRole::Vec1(1.0), "material id"),
        ]
    }

    fn outputs(&self) -> Vec<NodeTerminal> {
        vec![NodeTerminal::new(
            "output",
            NodeTerminalRole::Vec4(Vec4::zero()),
            "x = distance, w = material",
        )]
    }

    fn evaluate_3d(&self, pos: Vec3<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        let a = inputs[0].xyz();
        let b = inputs[1].xyz();
        let mut size = inputs[2].xyz();
        let modifier = inputs[3].x;

        let ab = b - a;
        let ab_len = ab.magnitude();
        let center = a + ab * 0.5;

        let z = ab / ab_len; // forward
        let x = if z.x.abs() < 0.99 {
            z.cross(Vec3::unit_x()).normalized()
        } else {
            z.cross(Vec3::unit_y()).normalized()
        };
        let y = z.cross(x);

        let basis = Mat3::new(x.x, y.x, z.x, x.y, y.y, z.y, x.z, y.z, z.z);
        let local_p = basis.transposed() * (pos - center);

        size.z = ab_len * 0.5; // half-length along AB

        let d = sdf_box(local_p, size) - modifier * 0.5;

        Vec4::broadcast(d - modifier * 0.5)
    }
}

/* ------------------------------------------------------------ */
/// Signed distance to an axis-aligned box of half-size `size`, centred at origin.
fn sdf_box(p: Vec3<F>, size: Vec3<F>) -> F {
    let q = p.map2(size, |a, b| a.abs() - b);

    // outside distance
    let outside = Vec3::new(q.x.max(0.0), q.y.max(0.0), q.z.max(0.0));
    // inside distance
    let inside = q.x.max(q.y).max(q.z).min(0.0);

    outside.magnitude() + inside
}
