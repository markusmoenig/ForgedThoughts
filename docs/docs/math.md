---
sidebar_position: 6
title: Math
---

# Math

Forge supports a small but growing set of scalar, vector, and noise helpers.

## Scalar and Vector Basics

- `vec3(x)` / `vec3(x, y, z)`: Builds a `vec3`; the one-argument form broadcasts the same value to all components.
- `dot(a, b)`: Dot product between two vectors.
- `length(v)`: Vector length.
- `normalize(v)`: Returns a unit-length version of the vector.
- `abs(x)`: Absolute value for scalars, or per-component absolute value for `vec3`.
- `min(a, b)`, `max(a, b)`: Scalar or per-component minimum/maximum.
- `clamp(x, a, b)`: Restricts a value or vector to a range.
- `mix(a, b, t)`: Blends between `a` and `b`; `0` gives `a`, `1` gives `b`.
- `step(edge, x)`: Returns `0` below the edge and `1` at or above it.
- `smoothstep(a, b, x)`: Smoothly blends from `0` at `a` to `1` at `b`.

```forge
let n = normalize(vec3(1.0, 2.0, 3.0));
let lit = max(dot(n, vec3(0.0, 1.0, 0.0)), 0.0);
let tint = mix(#44515f, #d3d9de, 0.35);
```

## Arithmetic and Trig

- `sin(x)`, `cos(x)`: Standard trigonometric functions.
- `sqrt(x)`: Square root.
- `floor(x)`, `ceil(x)`: Round down or up.
- `fract(x)`: Fractional part of a value.

These work well for masks, waves, and procedural breakup:

```forge
let bands = sin(p.y * 24.0);
let mask = smoothstep(0.2, 0.8, fract(p.x * 3.0));
```

## 3D Noise

- `value_noise_3d(p[, scale])`: Smooth scalar value noise in 3D; larger `scale` makes the noise field denser.
- `fbm_3d(p, octaves[, scale[, lacunarity]])`: Fractal 3D noise built from repeated value-noise octaves; more octaves add detail, `scale` sets the base frequency, and `lacunarity` controls how quickly frequency increases.

These are most useful in:

- material masks
- breakup and wear
- procedural dirt or moss coverage
- custom SDF logic

```forge
let broad = fbm_3d(ctx.local_position, 5.0, 1.2, 2.0);
let detail = value_noise_3d(ctx.local_position, 3.0);
let moss = smoothstep(0.35, 0.62, broad + detail * 0.15);
```

## Primitive Distance Calls

Forge also exposes native primitive SDF intrinsics with an object-oriented surface:

- `Box.distance(p, half_size)`: Signed distance to a box defined by half extents.
- `Sphere.distance(p, radius)`: Signed distance to a sphere.
- `Cylinder.distance(p, radius, half_height)`: Signed distance to a cylinder aligned to the canonical local bind axis.
- `Torus.distance(p, major_radius, minor_radius)`: Signed distance to a torus.
- `box_shell_sdf(p, half, wall, round)`: Hollow rounded box shell with preserved outer dimensions.
- `cylinder_x_sdf(p, radius, half_len)`, `cylinder_y_sdf(...)`, `cylinder_z_sdf(...)`: Axis-aligned cylinder SDF helpers.
- `hole_line_x_sdf(p, radius, half_len, spacing, count)`, `hole_line_y_sdf(...)`, `hole_line_z_sdf(...)`: Repeated axis-aligned cylindrical holes spaced along local `Z`.

These are especially useful inside custom Forge assets because they avoid repeating low-level shape code while staying on the native/JIT path.

```forge
let shell = Box.distance(p, vec3(0.4, 0.2, 0.8));
let cap = Sphere.distance(p - vec3(0.0, 0.0, 0.8), 0.18);
```
