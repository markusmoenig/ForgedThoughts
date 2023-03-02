use crate::prelude::*;

// PBR renderer, based on https://www.shadertoy.com/view/MlB3DV

fn g1v(dotnv: F, k: F) -> F {
	return 1.0 / (dotnv*(1.0 - k) + k);
}

fn compute_pbr_lighting(light: &Light, position: F3, n: F3, v: F3, albedo: F3, roughness: F, f0: F3 ) -> F3 {

	let alpha = roughness*roughness;
	let l = (light.position - position).normalize();
	let h = (v + l).normalize();

	let dotnl = (n.dot(&l)).clamp(0.0, 1.0);
	let dotnv = (n.dot(&v)).clamp(0.0, 1.0);
	let dotnh = (n.dot(&h)).clamp(0.0, 1.0);
	let dotlh = (l.dot(&h)).clamp(0.0, 1.0);

	// NDF : GGX
	let alpha_sqr = alpha*alpha;
    let denom = dotnh * dotnh * (alpha_sqr - 1.0) + 1.0;
	let d = alpha_sqr / (PI * denom * denom);

	// Fresnel (Schlick)
	let dotlh5 = (1.0 - dotlh).powf(5.0);
	let f = f0 + (F3::new_x(1.0) - f0).mult_f(&dotlh5);

	// Visibility term (G) : Smith with Schlick's approximation
	let k = alpha / 2.0;
	let vis = g1v(dotnl, k) * g1v(dotnv, k);

	let specular = /*dotNL **/ f.mult_f(&d).mult_f(&vis);

	let ambient = F3::new_x(0.01);

	let inv_pi = 0.31830988618;
	let diffuse = albedo.mult_f(&inv_pi);

	return ambient + (diffuse + specular) * light.rgb.mult_f(&dotnl);
}

#[inline(always)]
pub fn pbr(ctx: &FTContext, rd: &F3, hit: &HitRecord, color: &mut [f64; 4]) {

    let reflectance = 0.5;

    let f0 = F3::new_x(0.16 * reflectance*reflectance * (1.0-hit.material.metallic)) + hit.material.rgb.mult_f(&hit.material.metallic);
    let albedo = hit.material.rgb;

    //let mut s = 0.0;

    for light in &ctx.scene.lights {
		let col = compute_pbr_lighting(light, hit.hit_point, hit.normal, F3::new(-rd.x, -rd.y, -rd.z), albedo, hit.material.roughness, f0);

        color[0] += col.x;
        color[1] += col.y;
        color[2] += col.z;
    }

	// for ( int i = 0; i < NB_LIGHTS; ++i ) {
	// 	vec3 col = computePBRLighting ( lights[i], position, N, V, albedo, roughness, F0);
	// 	color += col;
    //     s += softshadow( position, normalize(lights[i].pos.xyz - position), 0.02, 2.5 );
	// }


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
        color[0] += hit.material.rgb.x * dif * l.intensity * occ;
        color[1] += hit.material.rgb.y * dif * l.intensity * occ;
        color[2] += hit.material.rgb.z * dif * l.intensity * occ;

        // Specular
        color[0] += ctx.settings.renderer.specular.x * dif * spe * occ;
        color[1] += ctx.settings.renderer.specular.y * dif * spe * occ;
        color[2] += ctx.settings.renderer.specular.z * dif * spe * occ;

        color[3] = 1.0;
    }
}