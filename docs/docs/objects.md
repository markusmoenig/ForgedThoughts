---
sidebar_position: 5
title: Objects
---

# Objects

Forge scenes are built from object literals, transforms, CSG composition, and custom SDF definitions.

## Built-in Primitives

Current built-ins include:

- `Sphere`
- `Box`
- `Cylinder`
- `Torus`
- `ExtrudePolygon`

Supported fields today:

`Sphere`

- `radius` or `r`
- `pos.x`, `pos.y`, `pos.z` or legacy `x`, `y`, `z`
- `rot.x`, `rot.y`, `rot.z` or legacy `rot_x`, `rot_y`, `rot_z`
- `material`

```ft
let ball = Sphere {
  radius: 1.0,
  material: Metal {
    color: #ebc757,
    roughness: 0.18
  }
};
```

`Box`

- `size: vec3(...)`
- `pos.*`
- `rot.*`
- `material`

```ft
let block = Box {
  size: vec3(1.2, 0.8, 1.2)
};
```

`Cylinder`

- `radius` or `r`
- `height` or `h`
- `pos.*`
- `rot.*`
- `material`

```ft
let column = Cylinder {
  radius: 0.5,
  height: 2.4
};
```

`Torus`

- `major_radius` or `R`
- `minor_radius` or `r`
- `pos.*`
- `rot.*`
- `material`

```ft
let ring = Torus {
  major_radius: 1.0,
  minor_radius: 0.2
};
```

`ExtrudePolygon`

- `sides` or `n` with a minimum of `3`
- `radius` or `r`
- `height` or `h`
- `pos.*`
- `rot.*`
- `material`

This is a regular N-gon extruded along the Y axis.

```ft
let hex = ExtrudePolygon {
  sides: 6,
  radius: 0.8,
  height: 0.35
};
```

Example:

```ft
var sphere = Sphere {
  radius: 1.0
};

sphere.pos.y = 0.6;
sphere.rot.z = 12.0;

let scene = sphere;
```

Transforms are currently driven with nested properties like `pos.x`, `pos.y`, `rot.x`, and `rot.z`.

Rounding is not yet a native primitive field like `Box { rounding: 0.2 }`. Right now rounded or beveled shapes are created with shape operators such as `.round(r)`.

## Shape Operators

For simple profile changes on a single shape, Forge supports:

- `shape.round(r)` for rounded/beveled forms

For boolean composition, see the dedicated [Booleans](./booleans.md) page.

## Custom SDFs

For geometry that goes beyond the built-ins, define a Forge SDF with a single distance contract:

```ft
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

## Why `bounds()` Matters

Without `bounds()`, custom SDFs fall back to a very conservative bound. That keeps rendering correct, but acceleration gets much worse and scenes can become noticeably slower.

Current `bounds()` behavior:

- it returns a local half-extent as `vec3(...)`, or a numeric radius
- the renderer currently turns that into a conservative bounding sphere radius
- that is already much better than the old giant fallback bound

So for most custom shapes, add `bounds()` early even if it is only approximate.

## Current Limits

- custom SDFs currently expose `distance(p)` and optional `bounds()`
- exact rotated bounds are not computed yet; the current implementation uses a conservative radius from `bounds()`
- custom SDF distance code is still interpreted, not VM/JIT compiled

## Imports and Reuse

Objects and SDFs can be shared through imports:

```ft
import "./shared/blob.ft";
import "SoftBlob" as blob;
```

Use relative imports for project-local assets and built-in library imports for reusable bundled definitions.
