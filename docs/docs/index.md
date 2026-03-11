---
sidebar_position: 1
title: Overview
---

# ForgedThoughts

ForgedThoughts is a CPU-first rendering project built around the `Forge` language, signed distance field scene construction, and programmable materials.

Today it has two rendering modes:

- the main renderer: a classical Whitted-style CPU renderer for fast lookdev
- `depth`: a fast depth preview for quick shape iteration

The renderer architecture is documented in the dedicated `Renderer` chapter.

What you can do with it right now:

- author `.ft` scene files in `Forge`
- build scenes from primitives, CSG operations, and custom SDFs
- light scenes with point lights and environment lights
- write materials with built-in backends like `Lambert`, `Metal`, and `Dielectric`
- tune renderer behavior through `RenderSettings`
- override material behavior with Forge-side hooks such as `color`, `roughness`, `ior`, `medium`, `eval`, `pdf`, and `sample`
- share materials, objects, scenes, and helper functions through imports, exports, and the built-in library

Where it is heading:

- richer programmable materials and geometry logic in Forge
- more reusable module and library structure
- a VM/JIT path for hot shading and procedural evaluation code
