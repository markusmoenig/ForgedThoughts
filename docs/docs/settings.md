---
sidebar_position: 3
title: Settings
---

# Settings

Forge scenes can provide a top-level `RenderSettings` object through `let render = RenderSettings { ... };`.

Example:

```forge
let render = RenderSettings {
  width: 800,
  height: 800,
  max_steps: 360,
  max_dist: 60.0,
  epsilon: 0.0002,
  step_scale: 0.7,
  accel: Bvh{}
};
```

## Core Render Settings

- `width`, `height`
  Output image size in pixels.

- `max_steps`
  Maximum raymarch steps per ray.

- `max_dist`
  Maximum trace distance before the ray is treated as a miss.

- `epsilon`
  Surface hit threshold for raymarching and a base scale for ray offsets.

- `step_scale`
  Multiplier applied to the raw SDF step size during marching.
  Default is `0.7`.
  Lower values are safer for non-ideal distance fields and smooth boolean scenes, but slower.

- `accel`
  Acceleration backend.
  Supported values:
  - `Naive{}`
  - `Bvh{}`
  - `Bricks{}`

## Notes

- CLI flags still override scene settings when both are provided.
- `step_scale` is currently important for scenes using smooth boolean operators, because not every operator behaves like a perfect conservative distance field yet.
