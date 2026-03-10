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

## CSG and Shape Operators

Objects can be combined with:

- `a + b` for union
- `a - b` for subtraction
- `shape.smooth(k)` for smooth union-style blending
- `shape.round(r)` for rounded/beveled forms

Example:

```ft
let shell = Sphere { radius: 1.0 };
let cut = Box { size: vec3(1.2, 1.2, 1.2) };
let scene = (shell - cut).round(0.05);
```

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
