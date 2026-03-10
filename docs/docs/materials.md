---
sidebar_position: 5
title: Materials
---

# Materials

ForgedThoughts currently has two layers:

1. built-in host BSDF backends: `Lambert`, `Metal`, `Dielectric`
2. FT-defined material overrides and custom BSDF hooks

Example:

```ft
material SoftGold {
  model: Metal;
  color = vec3(0.92, 0.78, 0.34);

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

Current FT material hooks:

- `color`
- `roughness`
- `ior`
- `thin_walled`
- `emission_color`
- `emission_strength`
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

Current reality:

- `eval/pdf/sample` are integrated most cleanly in the path tracer
- the recursive `ray` renderer still has some built-in material branching
- `medium` already affects transmission through simple absorption
- `subsurface` is structured material data but is not yet consumed as real transport
