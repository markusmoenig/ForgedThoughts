---
sidebar_position: 4
title: Language
---

# Forge Language

Forge is object-like and incremental.

```ft
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
- built-ins such as `mix`, `clamp`, `step`, `smoothstep`, `dot`, `length`, `normalize`, `sin`, `cos`
- hard booleans with `+`, `-`, and `&`
- named SDF boolean variants such as `union_round`, `diff_chamfer`, and `intersect_stairs`
- material definitions with local bindings and functions
- environment definitions with local bindings and functions
- custom SDF definitions with `sdf Name { fn distance(p) { ... } }`

## Booleans

Forge uses operators for the hard CSG core:

```ft
let shape = (a + b) - c;
let mask = a & b;
```

And named methods for the richer `hg_sdf`-style variants:

```ft
let shape =
  body
    .union_round(ring, 0.08)
    .diff_chamfer(cut, 0.04)
    .intersect_stairs(mask, 0.12, 6.0);
```

## Functions

Material functions can now also drive shading-normal perturbation with `fn normal(ctx) { ... }`, which is useful for procedural bump-style detail without changing the underlying SDF geometry.

Top-level helper functions can be reused across a module and may take multiple arguments:

```ft
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

```ft
import "Gold" as metals;

fn make_highlight() {
  return metals.Gold {};
}
```

## Imports

Imports are resolved before evaluation:

```ft
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

```ft
let private_color = #ebc757;
material Gold {
  model: Metal;
  color = private_color;
  roughness = 0.18;
};

export { Gold };
```

If a file contains `export { ... };`, imports treat that list as the intended public entry points.

## Environments

Procedural environments use the same block-style function model:

```ft
environment Sky {
  let zenith = #4d74c7;
  let horizon = #d8e7ff;

  fn color(dir) {
    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(horizon, zenith, t);
  }
};
```

`color(dir)` is called on ray misses in `render`, `ray`, and `path`.

## Status

The language is intentionally still small. Semantics are being stabilized before a VM/JIT layer is added.
