---
sidebar_position: 6
title: Lights
---

# Lights

Forge currently supports two built-in light types:

- `PointLight`
- `EnvLight`

## PointLight

Preferred form:

```ft
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

## EnvLight

Preferred form:

```ft
let sky = EnvLight {
  color: #d8e7ff,
  intensity: 0.35
};
```

Supported fields:

- `color`
- `intensity`

The environment contribution is `color * intensity`.
