---
sidebar_position: 5
title: Materials
---

# Materials

Forge now has a high-level default material model for normal authoring:

1. `Standard`: the default "uber" material for most work
2. built-in specialized backends: `Lambert`, `Metal`, `Dielectric`
3. low-level BSDF hooks for advanced custom work

Most materials should use `Standard` and drive its parameters procedurally when needed. BSDF hooks like `eval`, `pdf`, and `sample` are still available, but they are expert mode.

## Standard

`Standard` is the default authoring model. It gives you a single material with diffuse/specular/metal/transmission/clearcoat-style controls without forcing you to write BSDF math by hand.

Example:

```forge
material WetStone {
  color = #665e54;
  roughness = 0.48;
  metallic = 0.0;
  specular = 0.55;
  specular_weight = 1.0;
  specular_color = #ffffff;
  ior = 1.33;
  clearcoat_roughness = 0.07;

  fn clearcoat(ctx) {
    return smoothstep(-0.28, 0.08, sin(ctx.local_position.x) + cos(ctx.local_position.z)) * 0.22;
  }
}
```

Current Standard fields:

- `color`: Base surface color.
- `roughness`: Base roughness for the main lobe. `0` is sharp and glossy; `1` is broad and matte.
- `metallic`: Blends the material from dielectric toward metallic behavior. `0` looks like stone/plastic/wood; `1` looks like metal.
- `specular`: Base dielectric specular strength. `0` removes most glossy highlight; `1` makes the base highlight stronger.
- `specular_weight`: Extra multiplier for the specular lobe. `0` disables the base glossy lobe; `1` keeps it at full strength.
- `specular_color`: Tint for the specular response.
- `ior`: Index of refraction for dielectric and transmissive response. Lower values bend light less; higher values bend and reflect more strongly.
- `transmission`: Glass-like transmission amount. `0` is opaque; `1` is fully transmissive.
- `thin_walled`: Treats the material as a shell instead of a solid volume.
- `clearcoat`: Secondary glossy top-layer strength. `0` disables the coat; `1` gives a strong glossy top layer.
- `clearcoat_roughness`: Roughness of the clearcoat lobe. `0` is sharp and polished; `1` is soft and hazy.
- `emission_color`: Emitted light color.
- `emission_strength`: Emitted light intensity. `0` emits nothing; higher values make the material glow more strongly.
- `medium`: Interior absorption medium.
- `subsurface`: Structured subsurface data.
- `normal`: Full shading-normal override for expert bump/detail control.
- `bump`: Scalar height-style surface detail that automatically perturbs the shading normal. `0` leaves the surface smooth; higher values create more apparent raised or recessed detail without changing silhouette.

Material-local helper functions can also be reused across hooks, and they may take multiple arguments:

```forge
material BrickLike {
  model: Lambert;
  let brick = vec3(0.68, 0.24, 0.16);
  let mortar = vec3(0.8, 0.77, 0.72);

  fn blend(a, b, t) {
    return mix(a, b, t);
  }

  fn mortar_mask(ctx) {
    let band_x = smoothstep(-0.12, 0.12, sin(ctx.local_position.x * 11.0));
    let band_y = smoothstep(-0.12, 0.12, sin(ctx.local_position.y * 7.0));
    return max(band_x, band_y);
  }

  fn color(ctx) {
    return blend(brick, mortar, mortar_mask(ctx));
  }

  fn roughness(ctx) {
    return mix(0.7, 0.95, mortar_mask(ctx));
  }

  fn normal(ctx) {
    let mortar = mortar_mask(ctx);
    let groove_x = cos(ctx.local_position.x * 11.0) * 0.18;
    let groove_y = cos(ctx.local_position.y * 7.0) * 0.12;
    return normalize(ctx.normal + vec3(groove_x * mortar, groove_y * mortar, 0.0));
  }
};
```

Procedural material code can also use Forge's scalar 3D noise built-ins for masks and breakup:

```forge
fn moss_mask(ctx) {
  let broad = fbm_3d(ctx.local_position, 5.0, 1.2, 2.0);
  let detail = value_noise_3d(ctx.local_position, 3.0);
  return smoothstep(0.18, 0.42, broad * 0.7 + detail * 0.2);
}
```

Current Forge material hooks:

- `color`: Returns the surface color at the current hit point.
- `roughness`: Controls how sharp or broad the surface reflection looks.
- `ior`: Sets the index of refraction for dielectric materials.
- `thin_walled`: Treats the surface as a shell instead of a solid volume.
- `emission_color`: Sets the emitted light color.
- `emission_strength`: Scales how strongly the surface emits light.
- `normal`: Perturbs the shading normal for bump-style surface detail.
- `bump`: Returns a scalar bump field that the renderer turns into a shading-normal perturbation.
- `medium`: Describes the transmissive medium used inside the material.
- `subsurface`: Carries structured subsurface parameters for later transport use.
- `eval`: Evaluates how much light the material reflects for a given direction.
- `pdf`: Returns the sampling probability for the chosen BSDF direction.
- `sample`: Chooses the next ray direction and BSDF response for the bounce.

Current context fields include:

- `ctx.position`
- `ctx.local_position`
- `ctx.normal`
- `ctx.view_dir`
- `ctx.wo`
- `ctx.wi`
- `ctx.current_ior`
- `ctx.u1`, `ctx.u2`, `ctx.u3` inside `sample(ctx)`

## Advanced BSDF Hooks

Forge still supports direct BSDF hooks for advanced materials that need to override the renderer’s default scattering behavior.

Example:

```forge
material SoftGold {
  model: Metal;
  color = #ebc757;

  // Evaluate how much light this material reflects for a given incoming direction.
  fn eval(ctx) {
    let ndotl = max(dot(ctx.normal, ctx.wi), 0.0);
    return mix(vec3(0.08, 0.06, 0.03), color, ndotl) * (1.0 / 3.14159265);
  }

  // Return the probability density for the direction chosen by the sampler.
  fn pdf(ctx) {
    return max(dot(ctx.normal, ctx.wi), 0.0) / 3.14159265;
  }

  // Pick a new reflected or transmitted direction for the next ray bounce.
  fn sample(ctx) {
    return BsdfSample {
      wi: ctx.normal,
      f: color * (1.0 / 3.14159265),
      pdf: 1.0,
      delta: 0.0,
      apply_cos: 1.0,
      transmission: 0.0,
      thin_walled: 0.0,
      next_ior: ctx.current_ior
    };
  }
};
```

## Layered

Forge also supports a simple two-layer coated workflow on top of `Standard`.

If a material defines:

- `coat_color`
- `coat_roughness`
- `coat_ior`
- `coat_weight`
- `coat_mask(ctx)`

then the renderer treats it as:

- a base material driven by `color`, `roughness`, `metallic`, `specular`, `transmission`, and related hooks
- plus a glossy top coat driven by the `coat_*` fields

The coat does not replace the base material. It adds a second glossy layer over it, usually for:

- wet surfaces
- varnish
- clearcoat
- damp stone

Example:

```forge
material WetStone {
  color = #665e54;
  roughness = 0.5;

  coat_color = #ffffff;
  coat_roughness = 0.07;
  coat_ior = 1.33;
  coat_weight = 0.22;

  fn coat_mask(ctx) {
    return smoothstep(-0.28, 0.08, sin(ctx.local_position.x) + cos(ctx.local_position.z));
  }
}
```

In practice:

- use `clearcoat` when you want a simple uniform glossy top layer
- use the `coat_*` fields when you want a more explicitly layered wet/clearcoat workflow driven by masks

Materials can also be imported from disk or the built-in library:

```forge
import "./shared/brick.ft";
import "Gold" as gold;
```

Then use the imported definitions explicitly:

```forge
let scene = Sphere {
  material: gold.Gold {}
};
```

Built-in and user-defined materials can also be instantiated with per-instance overrides:

```forge
let floor = Box {
  size: vec3(20.0, 0.5, 20.0),
  material: CheckerFloor {
    color_a: #f5f6f8,
    color_b: #8b93a1,
    scale: 4.2
  }
};
```

Those override values are visible to the material's default properties, helper bindings, and runtime hooks like `fn color(ctx)`.

The built-in library currently exposes:

- `materials/...`
- `objects/...`
- `scenes/...`

Built-ins can be imported either by full path or by bare library name:

```forge
import "Glass";
import "materials/gold.ft";
```

For color-valued material fields, Forge also accepts hex literals as a shorthand for `vec3(...)`:

```forge
let gold = Metal {
  color: #ebc757,
  roughness: 0.18
};
```

Current reality:

- the renderer still has some built-in material branching
- `normal(ctx)` and `bump(ctx)` perturb the shading normal only; they do not change SDF intersections or silhouettes
- material-local helper functions can be reused across hooks like `color`, `roughness`, and `normal`
- `medium` already affects transmission through simple absorption
- `subsurface` is structured material data but is not yet consumed as real transport

Use `normal(ctx)` for things like:

- brick and mortar relief
- grooves
- fine surface breakup

Use `bump(ctx)` when you want a simpler height-style surface-detail workflow and do not want to build the full perturbed normal yourself.

True displacement that changes the actual SDF shape is a later feature.
