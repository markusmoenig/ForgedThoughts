---
sidebar_position: 1
title: Overview
---

# ForgedThoughts

ForgedThoughts is a CPU-first rendering project built around the `Forge` language, signed distance field scene construction, and programmable materials.

Today it has three distinct rendering paths:

- `ray`: a classical Whitted-style ray tracer for fast lookdev
- `path`: a progressive Monte Carlo path tracer for lighting and material validation
- `render`: a fast shaded preview renderer for quick material and shape iteration

What you can do with it right now:

- author `.ft` scene files in `Forge`
- build scenes from primitives, CSG operations, and custom SDFs
- write materials with built-in backends like `Lambert`, `Metal`, and `Dielectric`
- override material behavior with Forge-side hooks such as `color`, `roughness`, `ior`, `medium`, `eval`, `pdf`, and `sample`
- share materials, objects, scenes, and helper functions through imports, exports, and the built-in library

Where it is heading:

- richer programmable materials and geometry logic in Forge
- more reusable module and library structure
- a VM/JIT path for hot shading and procedural evaluation code
