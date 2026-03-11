---
sidebar_position: 6
title: Lights
---

# Lights

Forge currently supports three built-in light types:

- `PointLight`
- `SphereLight`
- `EnvLight`

## PointLight

Preferred form:

```forge
let key = PointLight {
  position: vec3(3.4, 4.0, 4.8),
  color: #fff2d8,
  intensity: 120.0
};
```

Supported fields:

- `position`
- `color`
- `intensity`

The emitted power is `color * intensity`, with inverse-square falloff from the light position.

## SphereLight

`SphereLight` is the first real area light in ForgedThoughts. It gives softer shadows than `PointLight` by sampling a spherical emitter.

```forge
let key = SphereLight {
  position: vec3(2.6, 3.4, 3.8),
  radius: 0.7,
  color: #fff1da,
  intensity: 48.0,
  samples: 8
};
```

Supported fields:

- `position`
- `radius` or `r`
- `color`
- `intensity`
- `samples`

Notes:

- The emitted power is `color * intensity`.
- Larger `radius` values produce softer shadows.
- Higher `samples` values reduce shadow stepping/noise, but cost more render time.
- If `radius` is `0`, `SphereLight` behaves like a point light.

## EnvLight

Preferred form:

```forge
let sky = EnvLight {
  color: #d8e7ff,
  intensity: 0.35
};
```

Supported fields:

- `color`
- `intensity`

The environment contribution is `color * intensity`.
