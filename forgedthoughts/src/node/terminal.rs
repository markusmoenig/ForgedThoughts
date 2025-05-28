use crate::F;
use vek::{Vec2, Vec3, Vec4};

/// The role and default value for a NodeTerminal.
#[derive(Debug, Clone)]
pub enum NodeTerminalRole {
    Vec1(F),
    Vec2(Vec2<F>),
    Vec3(Vec3<F>),
    Vec4(Vec4<F>),
}

impl NodeTerminalRole {
    pub fn type_name(&self) -> &'static str {
        match self {
            NodeTerminalRole::Vec1(_) => "float",
            NodeTerminalRole::Vec2(_) => "vec2",
            NodeTerminalRole::Vec3(_) => "vec3",
            NodeTerminalRole::Vec4(_) => "vec4",
        }
    }

    pub fn len(&self) -> usize {
        match self {
            NodeTerminalRole::Vec1(_) => 1,
            NodeTerminalRole::Vec2(_) => 2,
            NodeTerminalRole::Vec3(_) => 3,
            NodeTerminalRole::Vec4(_) => 4,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Extract the role from an vec4 with the given swizzle
    pub fn extract_from_vec4(&self, vec: Vec4<F>, swizzle: &str) -> Self {
        fn get_swizzled_component(vec: Vec4<F>, ch: char) -> F {
            match ch {
                'x' => vec.x,
                'y' => vec.y,
                'z' => vec.z,
                'w' => vec.w,
                _ => 0.0,
            }
        }
        match self {
            NodeTerminalRole::Vec1(_) => {
                let v = get_swizzled_component(vec, swizzle.chars().next().unwrap_or('x'));
                NodeTerminalRole::Vec1(v)
            }
            NodeTerminalRole::Vec2(_) => {
                let chars: Vec<char> = swizzle.chars().collect();
                let v = Vec2::new(
                    get_swizzled_component(vec, *chars.first().unwrap_or(&'x')),
                    get_swizzled_component(vec, *chars.get(1).unwrap_or(&'y')),
                );
                NodeTerminalRole::Vec2(v)
            }
            NodeTerminalRole::Vec3(_) => {
                let chars: Vec<char> = swizzle.chars().collect();
                let v = Vec3::new(
                    get_swizzled_component(vec, *chars.first().unwrap_or(&'x')),
                    get_swizzled_component(vec, *chars.get(1).unwrap_or(&'y')),
                    get_swizzled_component(vec, *chars.get(2).unwrap_or(&'z')),
                );
                NodeTerminalRole::Vec3(v)
            }
            NodeTerminalRole::Vec4(_) => NodeTerminalRole::Vec4(vec),
        }
    }
}

/// An input or output terminal (parameter) for a Node.
#[derive(Debug, Clone)]
pub struct NodeTerminal {
    pub name: String,

    /// Holds the role and default value of the terminal
    pub role: NodeTerminalRole,

    /// The swizzle of the terminal. As inter node communication is always vec4, we need to be able to decode the position
    /// of the terminal, like "zw".
    pub swizzle: String,
}

impl NodeTerminal {
    pub fn new(
        name: impl Into<String>,
        role: NodeTerminalRole,
        swizzle: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            role,
            swizzle: swizzle.into(),
        }
    }

    pub fn with_swizzle_offset(
        name: impl Into<String>,
        role: NodeTerminalRole,
        offset: usize,
    ) -> Self {
        let swizzle_chars = ['x', 'y', 'z', 'w'];

        let len = role.len(); // 1–4
        let swizzle = swizzle_chars
            .iter()
            .cycle()
            .skip(offset)
            .take(len)
            .collect::<String>();

        Self {
            name: name.into(),
            role,
            swizzle,
        }
    }
}
