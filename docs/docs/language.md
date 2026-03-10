---
sidebar_position: 4
title: Language
---

# FT Language

FT is object-like and incremental.

```ft
var sphere = Sphere {
  radius: 1.0
};

sphere.pos.y = 0.4;

let mat = Metal {
  color: vec3(0.92, 0.78, 0.34),
  roughness: 0.18
};

sphere.material = mat;
let scene = sphere;
```

Current supported pieces include:

- `let` and `var`
- nested property assignment like `pos.x` and `rot.z`
- object literals
- scalar and `vec3` arithmetic
- built-ins such as `mix`, `clamp`, `step`, `smoothstep`, `dot`, `length`, `normalize`, `sin`, `cos`
- material definitions with local bindings and functions

Material functions can now also drive shading-normal perturbation with `fn normal(ctx) { ... }`, which is useful for procedural bump-style detail without changing the underlying SDF geometry.

The language is intentionally still small. Semantics are being stabilized before a VM/JIT layer is added.
