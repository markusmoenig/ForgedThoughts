---
sidebar_position: 4
title: Overview
---

# Forge Language Overview

Forge is object-like and incremental.

```forge
var sphere = Sphere {
  radius: 1.0
};

sphere.pos.y = 0.4;

let mat = Metal {
  color: #ebc757,
  roughness: 0.18
};

sphere.material = mat;
let scene = sphere;
```

## Current Scope

Current supported pieces include:

- `let` and `var`
- top-level `fn name(...) { ... }`
- top-level `import "..." ;`
- top-level `export { ... };`
- nested property assignment like `pos.x` and `rot.z`
- object literals
- scalar and `vec3` arithmetic
- hex color literals like `#ff0000` and `#f00`
- built-ins such as `mix`, `clamp`, `step`, `smoothstep`, `dot`, `length`, `normalize`, `sin`, `cos`, `value_noise_3d`, and `fbm_3d`
- hard booleans with `+`, `-`, and `&`
- named SDF boolean variants such as `union_round`, `diff_chamfer`, and `intersect_stairs`
- material definitions with local bindings and functions
- environment definitions with local bindings and functions
- custom SDF definitions with programmable hooks like `distance(p)`, optional `domain(p)`, and optional `distance_post(d, p)`
- skeleton definitions with explicit `joint` and `bone` declarations
- object layout methods for relative placement
- semantic part material assignment like `table.legs.material = ...`
- semantic part placement like `vase.attach(table.top, Top)`

## Execution Today

Forge is JIT-accelerated for performance-sensitive code paths.

In practice, that means hot custom SDF functions and supported dynamic material hooks can run much faster than a purely interpreted implementation, while still keeping authoring in Forge instead of forcing everything into native Rust code.

## Booleans

Forge uses operators for the hard CSG core:

```forge
let shape = (a + b) - c;
let mask = a & b;
```

And named methods for the richer `hg_sdf`-style variants:

```forge
let shape =
  body
    .union_round(ring, 0.08)
    .diff_chamfer(cut, 0.04)
    .intersect_stairs(mask, 0.12, 6.0);
```

## Modeling Helpers

Forge now has a first domain-helper slice for object-level SDF composition:

- `mirror_x()`, `mirror_y()`, `mirror_z()`: Mirrors an object across its local X, Y, or Z axis.
- `repeat_x(spacing, count)`, `repeat_y(...)`, `repeat_z(...)`: Repeats an object along one axis with a fixed spacing and finite count.
- `slice_x(min, max)`, `slice_y(...)`, `slice_z(...)`: Keeps only the part of the object between two local-space planes on one axis.
- `noise(octaves[, scale[, lacunarity]])`: Applies recursive subtractive FBM-style surface breakup to the object.

These helpers are lowered into native renderer structures before marching, so they do not depend on the interpreted hot path.

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

Forge also exposes 3D scalar noise helpers for material and SDF code:

```forge
let n = value_noise_3d(ctx.local_position, 1.5);
let f = fbm_3d(ctx.local_position, 5.0, 1.2, 2.0);
```

- `value_noise_3d(p[, scale])`: Samples smooth scalar value noise at a 3D point.
- `fbm_3d(p, octaves[, scale[, lacunarity]])`: Builds multi-octave 3D fractal noise from repeated value-noise samples.

For fully programmable shaping inside custom SDFs, use SDF hooks instead:

- `fn domain(p)` transforms point space before `distance(p)`
- `fn distance_post(d, p)` modifies the computed distance afterward
- built-ins like `rotate_x/y/z(v, deg)` work in those hooks and stay on the JIT-accelerated path when they fit the supported subset

You can also attach programmable hooks directly to ordinary objects:

```forge
var statue = Box {
  size: vec3(0.55, 1.5, 0.42)
};

statue.domain = fn(p) {
  return rotate_y(p, p.y * 18.0);
};

statue.distance_post = fn(d, p) {
  return abs(d + sin((p.y + 0.75) * 115.0) * 0.0045) - 0.028;
};
```

## Functions

Material functions can now also drive shading-normal perturbation with `fn normal(ctx) { ... }` or the simpler scalar `fn bump(ctx) { ... }`, which is useful for procedural surface detail without changing the underlying SDF geometry.

Top-level helper functions can be reused across a module and may take multiple arguments:

```forge
fn accent() {
  return #ebc757;
}

fn tint(base, amount) {
  return mix(base, vec3(1.0), amount);
}

fn make_gold() {
  return Metal {
    color: tint(accent(), 0.12),
    roughness: 0.18
  };
}
```

Imported helpers also work through aliases:

```forge
import "Gold" as metals;

fn make_highlight() {
  return metals.Gold {};
}
```

## Skeletons

Forge supports semantic `skeleton` assets with explicit joints, bones, and rigid part binding.

See [Skeletons](./skeletons.md) for the full syntax, the `bind(...)` workflow, and the built-in `Robot` / `RobotBody` assets.

## Layout

Forge now has a small layout layer on top of raw `pos.*` edits. This is meant for scene assembly, not for replacing the underlying SDF model.

Current layout methods include:

- `attach(other, Top|Bottom|Left|Right|Front|Back[, gap])`
- `attach(other, BackRightCorner)` and other built-in corner anchors
- `attach(other, "OtherAnchor", "SelfAnchor")` for explicit anchor-to-anchor placement
- `align_x/y/z(other, Center|Top|Bottom|Left|Right|Front|Back)`
- `right_of`, `left_of`, `on_top_of`, `below`, `in_front_of`, `behind`
- `offset_x/y/z`
- `rotate_x/y/z`
- `face_to(other[, Anchor])` to orient local forward (`+Z`) toward another object or anchor

Example:

```forge
var sphere = Sphere {
  radius: 0.82,
  material: sphere_mat
}
  .attach(floor, Top)
  .align_z(floor, Center)
  .offset_x(-0.28);

var box = Box {
  size: vec3(1.15, 1.15, 1.15),
  material: box_mat
}
  .right_of(sphere, -0.8)
  .align_z(sphere, Center)
  .attach(floor, Top + 0.3)
  .face_to(sphere)
  .rotate_z(10.0);
```

Anchor values such as `Top` and `Center` support inline offsets:

```forge
.attach(floor, Top + 0.1)
.align_z(floor, Center - 0.4)
```

Built-in corner anchors work well for semantic room/object placement:

```forge
var cupboard = Box { size: vec3(2.0, 3.0, 1.5) }
  .attach(room, BackRightCorner);
```

For asset-specific placement, objects can expose custom local anchors and attach to them by name:

```forge
let character = Box {
  size: vec3(2.0, 4.0, 1.0),
  anchors: {
    FootLeft: vec3(-0.4, -2.0, 0.0)
  }
};

var shoe = Box {
  size: vec3(0.8, 0.4, 1.0),
  anchors: {
    Mount: vec3(0.0, -0.2, 0.0)
  }
}
  .attach(character, "FootLeft", "Mount");
```

Semantics:

- `attach(...)` chooses the contacting face relationship
- corner anchors align matching bottom/top/back/front/left/right corners
- explicit string anchors align named local anchor points between assets
- `align_*` only affects one axis at a time
- `right_of` and similar helpers only define that one relative direction
- `face_to(...)` sets `rot.x` and `rot.y` so local `+Z` points toward the target
- extra offsets are still explicit

So `right_of(a, -0.8)` does not silently imply matching `y` or `z`.

## Semantic Parts

Some lowered semantic assets expose named parts for assignment-friendly authoring.

Example:

```forge
var table = Table {
  width: 1.7,
  depth: 0.9,
  height: 0.78
};

table.top.material = Lambert { color: #7a4c35 };
table.legs.material = Metal { color: #2b3138, roughness: 0.22 };
```

Current part-oriented assignment support focuses on material overrides for semantic assets such as:

- `table.top.material`
- `table.legs.material`
- `cupboard.body.material`
- `cupboard.door.material`
- `lamp.body.material`
- `lamp.shade.material`
- `lamp.bulb.material`

The same semantic parts can also act as layout targets:

```forge
var vase = Sphere { radius: 0.18 }
  .attach(table.top, Top)
  .offset_x(-0.35);

var lamp = Lamp {}
  .attach(cupboard.body, Top, Bottom)
  .offset_x(0.42);
```

## Imports

Imports are resolved before evaluation:

```forge
import "./shared/materials.ft";
import "Gold" as gold;
import "SoftBlob" as blob;
```

Import rules:

- `./...` and `../...` import from disk relative to the current file
- `materials/...`, `objects/...`, and `scenes/...` import from the built-in embedded library
- bare built-in names like `Glass`, `SoftBlob`, and `Studio` also resolve from the embedded library
- `as name` namespaces the imported top-level symbols under `name.`
- imports are loaded once even if multiple files reference them
- cyclic imports are rejected

Files can also declare an explicit public surface:

```forge
let private_color = #ebc757;
material Gold {
  model: Metal;
  color = private_color;
  roughness = 0.18;
};

export { Gold };
```

If a file contains `export { ... };`, imports treat that list as the intended public entry points.

## Asset Metadata

Reusable library assets can carry their own metadata directly in the definition block.

For materials:

```forge
material Gold {
  name: "Gold";
  description: "Polished gold metal with moderate roughness.";
  tags: ["material", "metal", "gold", "reflective", "warm"];
  params: [
    {
      name: "roughness",
      type: "number",
      description: "Surface micro-roughness.",
      default: 0.18,
      min: 0.0,
      max: 1.0
    }
  ];

  model: Metal;
  color = #ebc757;
  roughness = 0.18;
};
```

For custom SDF objects:

```forge
sdf SoftBlob {
  name: "SoftBlob";
  description: "Warped blob SDF with a conservative bounds helper.";
  tags: ["object", "sdf", "blob", "organic"];

  fn bounds() {
    return vec3(1.2, 1.2, 1.1);
  }

  fn distance(p) {
    return length(p) - 1.0;
  }
};
```

Supported metadata fields today:

- `name: "..." ;`
- `description: "..." ;`
- `tags: ["...", "..."] ;`
- `params: [ { ... }, { ... } ] ;`

Parameter metadata is intended for library discovery and future editors/AI tools. A parameter entry can describe:

- `name`
- `type`
- `description`
- `default`
- `min`
- `max`

These fields are intended for library discovery, tooling, and future AI-driven scene composition. They do not change rendering behavior directly.

## Environments

Procedural environments use the same block-style function model:

```forge
environment Sky {
  let zenith = #4d74c7;
  let horizon = #d8e7ff;

  fn color(dir) {
    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(horizon, zenith, t);
  }
};
```

`color(dir)` is called on ray misses in the main renderer and in `depth`.

## Status

The language is intentionally still small. Semantics are being stabilized before a VM/JIT layer is added.
