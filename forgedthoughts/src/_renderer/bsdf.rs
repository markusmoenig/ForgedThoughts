use crate::prelude::*;

use rust_pathtracer::prelude::*;
use rhai::{Scope};

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

    fn background(&self, ray: &Ray) -> F3 {
        let mut color = F3::zeros();
        let mut color_is_set = false;

        if let Some(ctx) = &self.ctx {
            if ctx.settings.background_fn {

                let mut s = Scope::new();
                let rc = ctx.engine.call_fn::<F3>(&mut s, &ctx.ast, "background", ( (ray.clone()), ) );

                if rc.is_err() {
                    println!("{}", rc.err().unwrap().to_string());
                } else
                if let Some(bc) = rc.ok() {
                    color = bc;
                    color_is_set = true;
                }
            }

            if color_is_set == false {
                color = ctx.settings.background;
            }
        }

        color
    }

    /// The closest hit, includes light sources.
    fn closest_hit(&self, ray: &Ray, state: &mut State, light_sample: &mut LightSampleRec) -> bool {

        let mut hit = false;

        if let Some(hit_record) = self.ctx.as_ref().unwrap().scene.raymarch(&ray, &mut self.ctx.as_ref().unwrap()) {
            hit = true;

            state.hit_dist = hit_record.distance;
            state.normal = hit_record.normal;

            state.material = hit_record.material;
        }

        if self.sample_lights(ray, state, light_sample, &self.lights) {
            hit = true;
        }

        hit
    }

    /// Any hit
    fn any_hit(&self, ray: &Ray, _max_dist: F) -> bool {
        self.ctx.as_ref().unwrap().scene.shadow_march(&ray, &mut self.ctx.as_ref().unwrap())
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

pub trait FTScene<'a> : Sync + Send {

    fn new_ctx(ctx: FTContext<'a>) -> BSDFScene<'a> where Self: Sized;
}

impl<'a>  FTScene<'a> for BSDFScene<'a> {
    fn new_ctx(ctx: FTContext<'a> ) -> BSDFScene<'a>  {

        let mut lights = vec![];
        let light_scale = 1.0;

        for light in &ctx.scene.lights {

            let position = light.position;
            let emission = F3::new(
                light.rgb.x * light.intensity * light_scale,
                light.rgb.y * light.intensity * light_scale,
                light.rgb.z * light.intensity * light_scale);

            let l = AnalyticalLight::spherical(position, light.radius, emission);

            lights.push(l);
        }

        let mut pinhole = Box::new(Pinhole::new());

        pinhole.set(ctx.camera.origin, ctx.camera.center);
        pinhole.set_fov(ctx.camera.fov);

        Self {
            ctx             : Some(ctx),
            lights,
            pinhole
        }
    }
}

/*
// Analytical Intersections

impl<'a> AnalyticalIntersections<'a> for BSDFScene<'a> {

    // Based on https://www.scratchapixel.com/lessons/3d-basic-rendering/minimal-ray-tracer-rendering-simple-shapes/ray-sphere-intersection
    fn sphere(&self, ray: &Ray, center: F3, radius: F) -> Option<F> {
        let l = center - ray.origin;
        let tca = l.dot(&ray.direction);
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
    fn plane(&self, ray: &Ray) -> Option<F> {
        let normal = F3::new(0.0, 1.0, 0.0);
        let denom = dot(&normal, &ray.direction);

        if denom.abs() > 0.0001 {
            let t = dot(&(F3::new(0.0, -1.0, 0.0) - ray.origin), &normal) / denom;
            if t >= 0.0 {
                return Some(t);
            }
        }
        None
    }
}

#[allow(unused)]
pub trait AnalyticalIntersections<'a> : Sync + Send {

    fn sphere(&self, ray: &Ray, center: F3, radius: F) -> Option<F>;
    fn plane(&self, ray: &Ray) -> Option<F>;

}*/