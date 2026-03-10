---
sidebar_position: 5
title: Materials
---

# Materials

ForgedThoughts currently has two layers:

1. built-in host BSDF backends: `Lambert`, `Metal`, `Dielectric`
2. Forge-defined material overrides and custom BSDF hooks

Example:

```ft
material SoftGold {
  model: Metal;
  color = #ebc757;

  fn eval(ctx) {
    let ndotl = max(dot(ctx.normal, ctx.wi), 0.0);
    return mix(vec3(0.08, 0.06, 0.03), color, ndotl) * (1.0 / 3.14159265);
  }

  fn pdf(ctx) {
    return max(dot(ctx.normal, ctx.wi), 0.0) / 3.14159265;
  }

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

Material-local helper functions can also be reused across hooks:

```ft
material BrickLike {
  model: Lambert;
  let brick = vec3(0.68, 0.24, 0.16);
  let mortar = vec3(0.8, 0.77, 0.72);

  fn mortar_mask(ctx) {
    let band_x = smoothstep(-0.12, 0.12, sin(ctx.local_position.x * 11.0));
    let band_y = smoothstep(-0.12, 0.12, sin(ctx.local_position.y * 7.0));
    return max(band_x, band_y);
  }

  fn color(ctx) {
    return mix(brick, mortar, mortar_mask(ctx));
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

- `color`
- `roughness`
- `ior`
- `thin_walled`
- `emission_color`
- `emission_strength`
- `normal`
- `medium`
- `subsurface`
- `eval`
- `pdf`
- `sample`

Current context fields include:

- `ctx.position`
- `ctx.local_position`
- `ctx.normal`
- `ctx.view_dir`
- `ctx.wo`
- `ctx.wi`
- `ctx.current_ior`
- `ctx.u1`, `ctx.u2`, `ctx.u3` inside `sample(ctx)`

Materials can also be imported from disk or the built-in library:

```ft
import "./shared/brick.ft";
import "Gold" as gold;
```

Then use the imported definitions explicitly:

```ft
let scene = Sphere {
  material: gold.Gold {}
};
```

The built-in library currently exposes:

- `materials/...`
- `objects/...`
- `scenes/...`

Built-ins can be imported either by full path or by bare library name:

```ft
import "Glass";
import "materials/gold.ft";
```

For color-valued material fields, Forge also accepts hex literals as a shorthand for `vec3(...)`:

```ft
let gold = Metal {
  color: #ebc757,
  roughness: 0.18
};
```

Current reality:

- `eval/pdf/sample` are integrated most cleanly in the path tracer
- the recursive `ray` renderer still has some built-in material branching
- `normal(ctx)` perturbs the shading normal only; it does not change SDF intersections or silhouettes
- material-local helper functions can be reused across hooks like `color`, `roughness`, and `normal`
- `medium` already affects transmission through simple absorption
- `subsurface` is structured material data but is not yet consumed as real transport

Use `normal(ctx)` for things like:

- brick and mortar relief
- grooves
- fine surface breakup

True displacement that changes the actual SDF shape is a later feature.
