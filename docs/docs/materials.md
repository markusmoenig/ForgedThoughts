---
sidebar_position: 5
title: Materials
---

# Materials

ForgedThoughts currently has two layers:

1. built-in host BSDF backends: `Lambert`, `Metal`, `Dielectric`
2. Forge-defined material overrides and custom BSDF hooks

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

Current Forge material hooks:

- `color`: Returns the surface color at the current hit point.
- `roughness`: Controls how sharp or broad the surface reflection looks.
- `ior`: Sets the index of refraction for dielectric materials.
- `thin_walled`: Treats the surface as a shell instead of a solid volume.
- `emission_color`: Sets the emitted light color.
- `emission_strength`: Scales how strongly the surface emits light.
- `normal`: Perturbs the shading normal for bump-style surface detail.
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

## Layered

Forge also has a constrained two-layer material model for common cases like wet stone, clearcoat, or varnish.

Example:

```forge
material WetStone {
  model: Layered;

  color = #5f5951;
  roughness = 0.56;

  coat_color = #ffffff;
  coat_roughness = 0.05;
  coat_ior = 1.33;
  coat_weight = 0.6;

  fn coat_mask(ctx) {
    return smoothstep(-0.1, 0.2, sin(ctx.local_position.x) + cos(ctx.local_position.z));
  }
};
```

Current layered fields:

- `color`: Base layer color.
- `roughness`: Base layer roughness.
- `coat_color`: Top coat color, usually near white.
- `coat_roughness`: Coat roughness for glossy or wet response.
- `coat_ior`: Coat index of refraction.
- `coat_weight`: Overall coat strength.
- `coat_mask`: Per-hit mask for where the coat appears.

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
- `normal(ctx)` perturbs the shading normal only; it does not change SDF intersections or silhouettes
- material-local helper functions can be reused across hooks like `color`, `roughness`, and `normal`
- `medium` already affects transmission through simple absorption
- `subsurface` is structured material data but is not yet consumed as real transport

Use `normal(ctx)` for things like:

- brick and mortar relief
- grooves
- fine surface breakup

True displacement that changes the actual SDF shape is a later feature.
