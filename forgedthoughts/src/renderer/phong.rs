use crate::prelude::*;

#[inline(always)]
/// Phong renderer, based on https://www.shadertoy.com/view/XlXGDj
pub fn phong(ctx: &FTContext, rd: &F3, hit: &HitRecord, color: &mut [F; 4]) {
    for l in &ctx.scene.lights {
        let light_dir = l.position - hit.hit_point;

        let occ = 0.5 + 0.5 * hit.normal.y;
        let amb = occ.clamp(0.0, 1.0);
        let dif = hit.normal.dot(&light_dir).clamp(0.0, 1.0);

        let h = (F3::new(-rd.x, -rd.y, -rd.z) + light_dir).normalize();
        let spe = h.dot(&hit.normal).clamp(0.0, 1.0).powf(64.0);

        // Ambient
        color[0] += ctx.settings.renderer.ambient.x * amb * occ;
        color[1] += ctx.settings.renderer.ambient.y * amb * occ;
        color[2] += ctx.settings.renderer.ambient.z * amb * occ;

        // Diffuse
        color[0] += ctx.scene.sdfs[hit.sdf_index].material.rgb.x * dif * l.intensity * occ;
        color[1] += ctx.scene.sdfs[hit.sdf_index].material.rgb.y * dif * l.intensity * occ;
        color[2] += ctx.scene.sdfs[hit.sdf_index].material.rgb.z * dif * l.intensity * occ;

        // Specular
        color[0] += ctx.settings.renderer.specular.x * dif * spe * occ;
        color[1] += ctx.settings.renderer.specular.y * dif * spe * occ;
        color[2] += ctx.settings.renderer.specular.z * dif * spe * occ;

        color[3] = 1.0;
    }
}