---
sidebar_position: 5
title: Modeling
---

# Modeling

Forge supports a growing set of modeling helpers for building custom objects without dropping fully into low-level SDF math every time.

The current model has three layers:

- object-level modeling helpers like mirrors, repeats, slices, and carved noise
- native primitive distance intrinsics you can call from Forge
- fully programmable custom SDF hooks such as `domain(p)` and `distance_post(d, p)`

## Object Helpers

Forge exposes a first object-level modeling-helper slice:

- `mirror_x()`, `mirror_y()`, `mirror_z()`: Mirrors an object across its local X, Y, or Z axis.
- `repeat_x(spacing, count)`, `repeat_y(...)`, `repeat_z(...)`: Repeats an object along one axis with fixed spacing and finite count.
- `slice_x(min, max)`, `slice_y(...)`, `slice_z(...)`: Clips an object to a local-space range on one axis.
- `noise(octaves[, scale[, lacunarity]])`: Applies recursive subtractive FBM-style breakup to the object surface.
- `hole_line_x_sdf(p, radius, half_len, spacing, count)`, `hole_line_y_sdf(...)`, `hole_line_z_sdf(...)`: Builds a repeated line of cylindrical holes along local `Z`, useful for perforated parts and vents in custom assets.

```forge
let rib = Box { size: vec3(0.2, 1.0, 0.4) };

let columns = rib.repeat_x(0.6, 5.0);
let mirrored = columns.mirror_z();
let clipped = mirrored.slice_y(-0.4, 0.4);
```

`noise(...)` is useful for carved rock and terrain-like breakup on otherwise simple shapes:

```forge
let stone = Box {
  size: vec3(1.0, 1.0, 1.0),
  round: 0.08
}
  .noise(7.0, 1.6, 1.2);
```

These helpers are lowered into native renderer structures before marching, so they do not depend on the slow interpreted hot path.

## Math Helpers

Forge’s general scalar, vector, 3D noise, and native primitive distance helpers now live on the dedicated [Math](./math.md) page.

## Programmable SDF Hooks

For geometry that goes beyond the built-ins, define a Forge SDF with a single distance contract:

```forge
sdf SoftBlob {
  fn bounds() {
    return vec3(1.2, 1.2, 1.1);
  }

  fn warp(p) {
    return vec3(p.x, p.y + sin(p.x * 4.0) * 0.16, p.z);
  }

  fn distance(p) {
    let q = warp(p);
    return length(q) - 1.0;
  }
};

let scene = SoftBlob {};
```

Rules:

- `fn distance(p)` is required
- `p` is evaluated in local/object space
- helper functions can be reused inside the SDF block
- `fn bounds()` is optional but strongly recommended
- optional `fn domain(p)` can transform point space before `distance(p)`
- optional `fn distance_post(d, p)` can modify the computed distance afterward

Example programmable modifier pattern:

```forge
sdf TwistStatue {
  fn bounds() {
    return vec3(0.4, 0.9, 0.4);
  }

  fn domain(p) {
    return rotate_y(p, p.y * 18.0);
  }

  fn distance(p) {
    return length(p) - 0.5;
  }

  fn distance_post(d, p) {
    return abs(d + sin(p.y * 120.0) * 0.004) - 0.03;
  }
}
```

The same programmable hooks can also be attached directly to ordinary objects:

```forge
var statue = Box { size: vec3(0.55, 1.5, 0.42) };

statue.domain = fn(p) {
  return rotate_y(p, p.y * 18.0);
};

statue.distance_post = fn(d, p) {
  return abs(d + sin((p.y + 0.75) * 115.0) * 0.0045) - 0.028;
};
```

## Bounds

Without `bounds()`, custom SDFs fall back to a conservative bound. That keeps rendering correct, but acceleration gets much worse and scenes can become noticeably slower.

Current `bounds()` behavior:

- it returns a local half-extent as `vec3(...)`, or a numeric radius
- the renderer caches that and uses it for pruning
- approximate bounds are still much better than none

So for most custom shapes, add `bounds()` early even if it is only approximate.
