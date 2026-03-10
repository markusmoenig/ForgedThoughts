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
- material definitions with local bindings and functions
- custom SDF definitions with `sdf Name { fn distance(p) { ... } }`

Material functions can now also drive shading-normal perturbation with `fn normal(ctx) { ... }`, which is useful for procedural bump-style detail without changing the underlying SDF geometry.

Top-level helper functions can be reused across a module:

```ft
fn accent() {
  return #ebc757;
}

fn make_gold() {
  return Metal {
    color: accent(),
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

The language is intentionally still small. Semantics are being stabilized before a VM/JIT layer is added.
