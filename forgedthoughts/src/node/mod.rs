pub mod graph;
pub mod terminal;

pub mod material;
pub mod nodes;
pub mod shapes;

use crate::{NodeTerminal, F};
use vek::{Vec2, Vec3, Vec4};

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum NodeRole {
    Node,
    Shape,
    Material,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum NodeDomain {
    D2,
    D3,
}

#[allow(unused)]
pub trait Node: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Returns the name of the node.
    fn name(&self) -> &str;

    /// Returns the role of the node.
    fn role(&self) -> NodeRole;

    /// Returns the domain of the node.
    fn domain(&self) -> NodeDomain;

    /// Returns the supported inputs of the node.
    fn inputs(&self) -> Vec<NodeTerminal>;

    /// Returns the supported outputs of the node.
    fn outputs(&self) -> Vec<NodeTerminal>;

    /// Evaluated for procedural color/image graphs (UV-based)
    fn evaluate_2d(&self, uv: Vec2<F>, resolution: Vec2<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        Vec4::zero() // Default: not implemented
    }

    /// Evaluated for shape modeling (position-based SDFs)
    fn evaluate_3d(&self, pos: Vec3<F>, inputs: &[Vec4<F>]) -> Vec4<F> {
        Vec4::broadcast(f32::MAX) // Default: not implemented
    }
}
