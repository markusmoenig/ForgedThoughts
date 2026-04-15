---
sidebar_position: 4
title: Language
---

# Forge Language (Node-Centric)

Forge (FT) is the node-definition language used by graph files.

Primary authoring split:

- FT: reusable node behavior
- TOML: node instancing/wiring/render selection

## Node Definition Shape

```forge
node ValueNoise {
  let scale = 1.0;
  let octaves = 4.0;

  fn eval(ctx) {
    let x = ctx.pos2d.x * scale;
    let z = ctx.pos2d.z * scale;
    return value_noise_2d(x, z);
  }
};
```

## Current Key Concepts

- `node Name { ... }`
- `let` bindings for defaults
- `fn eval(ctx)` as public entrypoint
- helper functions inside nodes (`fn hash(...)`, `fn average8(...)`, etc.)
- point/field node behavior via graph port typing

## Material-Lane Direction

Material nodes are expected to write explicit shading lanes rather than rely on opaque output blobs:

- `albedo`
- `roughness`
- `metallic`
- `coat`
- `coat_roughness`
- `transparency`
- optional `ior`, `emission`, shell lanes

See [MaterialNodeContract.md](/Users/markusmoenig/ForgedThoughts/MaterialNodeContract.md).

## Context Model

`eval(ctx)` uses an engine-defined context schema, not an arbitrary dynamic object.

Important lanes include:

- `ctx.pos2d`, `ctx.pos3d`
- `ctx.height`, `ctx.slope`, `ctx.curvature`
- `ctx.normal`, `ctx.view_dir`
- material output lanes (`ctx.albedo`, `ctx.roughness`, etc.)

See [NodeContextDesign.md](/Users/markusmoenig/ForgedThoughts/NodeContextDesign.md).

## Graph Integration

Node definitions are instantiated from TOML:

```toml
[ValueNoise.main]
scale = 3.0
octaves = 6.0
```

Then selected explicitly in `[render]`:

```toml
[render]
stage = "height"
target = "grayscale"
source = "ValueNoise.main:field"
```

For `scene/raytrace`, render binds one material object:

```toml
[render]
stage = "scene"
target = "raytrace"
source = "Terrain.main:field"
material = "Material.main:material"

[Material.main]
displacement = "SphereFbm.main:field"
base_color = "#8f7a5a"
max_extend = 0.25
roughness = 0.72
metallic = 0.0
coat = 0.0
coat_roughness = 0.0
transparency = 0.0
```

For terrain-style material rendering, `render.material` is optional.
If omitted, the renderer auto-collects unreferenced top-level `Material.*` nodes and evaluates them automatically using each material's own `height`, `height_band`, `slope`, `slope_band`, and `max_extend` settings.

## Legacy Note

Forge still contains older language surface for scene/SDF workflows, but current project direction is graph-driven terrain/material pipelines.
