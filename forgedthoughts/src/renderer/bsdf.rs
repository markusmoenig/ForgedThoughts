use crate::prelude::*;

use rust_pathtracer::prelude::*;

pub use nalgebra::*;
extern crate nalgebra_glm as glm;

pub struct BSDFScene<'a> {
    ctx                 : Option<FTContext<'a>>,
    lights              : Vec<AnalyticalLight>,
    pinhole             : Box<dyn Camera3D>,
}

// The Scene

impl rust_pathtracer::scene::Scene for BSDFScene<'_> {

    fn new() -> Self {

        // Not called
        let lights = vec![];
        let pinhole = Box::new(Pinhole::new());

        Self {
            ctx             : None,
            lights,
            pinhole
        }
    }

    fn camera(&self) -> &Box<dyn Camera3D> {
        &self.pinhole
    }

    fn background(&self, _ray: &Ray) -> PTF3 {
        self.ctx.as_ref().unwrap().settings.background.to_v3()
    }

    /// The closest hit, includes light sources.
    fn closest_hit(&self, ray: &Ray, state: &mut State, _light: &mut LightSampleRec) -> bool {

        let mut hit = false;

        state.depth = 0;

        let o = F3::from_v3(&ray[0]);
        let d = F3::from_v3(&ray[1]);

        if let Some(hit_record) = self.ctx.as_ref().unwrap().scene.raymarch(&o, &d, &mut self.ctx.as_ref().unwrap()) {
            hit = true;

            state.hit_dist = hit_record.distance;
            state.normal = hit_record.normal.to_v3();

            state.material.base_color = hit_record.material.rgb.to_v3();

            state.material.roughness = hit_record.material.roughness;
            state.material.metallic = hit_record.material.metallic;
        }

        hit
    }

    /// Any hit
    fn any_hit(&self, ray: &Ray, _max_dist: PTF) -> bool {

        let o = F3::from_v3(&ray[0]);
        let d = F3::from_v3(&ray[1]);

        self.ctx.as_ref().unwrap().scene.shadow_march(&o, &d, &mut self.ctx.as_ref().unwrap())
    }

    /// Returns the light at the given index
    fn light_at(&self, index: usize) -> &AnalyticalLight {
        &self.lights[index]
    }

    fn number_of_lights(&self) -> usize {
        self.lights.len()
    }

    /// The recursion depth for the path tracer
    fn recursion_depth(&self) -> u16 {
        self.ctx.as_ref().unwrap().settings.renderer.depth as u16
    }

}

// Analytical Intersections

impl AnalyticalIntersections for BSDFScene<'_> {

    // Based on https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-sphere-intersection
    fn sphere(&self, ray: &Ray, center: PTF3, radius: PTF) -> Option<PTF> {
        let l = center - ray[0];
        let tca = l.dot(&ray[1]);
        let d2 = l.dot(&l) - tca * tca;
        let radius2 = radius * radius;
        if d2 > radius2 {
            return None;
        }
        let thc = (radius2 - d2).sqrt();
        let mut t0 = tca - thc;
        let mut t1 = tca + thc;

        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }

        if t0 < 0.0 {
            t0 = t1;
            if t0 < 0.0 {
                return None;
            }
        }

        Some(t0)
   }

    // Ray plane intersection
    fn plane(&self, ray: &Ray) -> Option<PTF> {
        let normal = PTF3::new(0.0, 1.0, 0.0);
        let denom = glm::dot(&normal, &ray[1]);

        if denom.abs() > 0.0001 {
            let t = glm::dot(&(PTF3::new(0.0, -1.0, 0.0) - ray[0]), &normal) / denom;
            if t >= 0.0 {
                return Some(t);
            }
        }
        None
    }
}

#[allow(unused)]
pub trait AnalyticalIntersections : Sync + Send {

    fn sphere(&self, ray: &Ray, center: PTF3, radius: PTF) -> Option<PTF>;
    fn plane(&self, ray: &Ray) -> Option<PTF>;

}

pub trait FTScene<'a> : Sync + Send {

    fn new_ctx(ctx: FTContext<'a>) -> BSDFScene<'a> where Self: Sized;
}

impl<'a>  FTScene<'a> for BSDFScene<'a> {
    fn new_ctx(ctx: FTContext<'a> ) -> BSDFScene<'a>  {

        let mut lights = vec![];
        let light_scale = 80.0;

        for light in &ctx.scene.lights {

            let position = light.position.to_v3();
            let emission = PTF3::new(
                light.rgb.x * light.intensity * light_scale,
                light.rgb.y * light.intensity * light_scale,
                light.rgb.z * light.intensity * light_scale);

            let l = AnalyticalLight::spherical(position, light.radius, emission);

            lights.push(l);
        }

        let mut pinhole = Box::new(Pinhole::new());

        let origin = ctx.camera.origin.to_v3();
        let center = ctx.camera.center.to_v3();

        pinhole.set(origin, center);
        pinhole.set_fov(ctx.camera.fov);

        Self {
            ctx             : Some(ctx),
            lights,
            pinhole
        }
    }
}