use crate::F;
use vek::Vec3;

/// General purpose material class. Gets filled during graph material evaluation.
#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Material {
    pub albedo: Vec3<F>,
    pub emission: Vec3<F>,

    pub roughness: F,
    pub metallic: F,
    pub ior: F,
}

impl Default for Material {
    fn default() -> Self {
        Self::new()
    }
}

impl Material {
    pub fn new() -> Self {
        Self {
            albedo: Vec3::broadcast(0.5),
            emission: Vec3::zero(),

            roughness: 0.5,
            metallic: 0.0,
            ior: 1.45,
        }
    }

    /// Returns the albedo converted to linear color space using approximate sRGB to linear conversion.
    pub fn albedo_linear(&self) -> Vec3<F> {
        fn srgb_to_linear(c: F) -> F {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }

        Vec3::new(
            srgb_to_linear(self.albedo.x),
            srgb_to_linear(self.albedo.y),
            srgb_to_linear(self.albedo.z),
        )
    }
}
