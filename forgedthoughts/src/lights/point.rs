use crate::prelude::*;
use vek::Vec3;

use crate::lights::Light;

pub struct PointLight {
    pub position: Vec3<F>,
    pub color: Vec3<F>,
    pub intensity: F,
    pub radius: F,
}

impl Light for PointLight {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self {
            position: Vec3::new(0.0, 0.5, 0.0),
            color: Vec3::one(),
            radius: 1.0,
            intensity: 1.0,
        }
    }

    fn name(&self) -> &str {
        "Pinhole"
    }

    fn position(&self) -> Vec3<F> {
        self.position
    }
    fn set_position(&mut self, position: Vec3<F>) {
        self.position = position;
    }

    fn color(&self) -> Vec3<F> {
        self.color
    }

    fn set_color(&mut self, color: Vec3<F>) {
        self.color = color;
    }

    fn radius(&self) -> F {
        self.radius
    }

    fn set_radius(&mut self, radius: F) {
        self.radius = radius;
    }

    fn intensity(&self) -> F {
        self.intensity
    }

    fn set_intensity(&mut self, intensity: F) {
        self.intensity = intensity;
    }
}
