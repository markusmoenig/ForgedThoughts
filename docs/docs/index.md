---
sidebar_position: 1
title: Overview
---

# ForgedThoughts Overview

ForgedThoughts is now a terrain-first graph system:

- FT for reusable node definitions
- TOML for graph instancing/wiring
- Whitted terrain raymarching driven by graph outputs

The primary path is no longer old scene-centric authoring.  
The primary path is **graph-driven terrain/material workflows**.

## Core Pieces

1. **Node definitions (FT)**
   - `node ValueNoise { ... }`
   - `fn eval(ctx)` entrypoint
2. **Graph files (TOML)**
   - `[Type.alias]` instances
   - explicit port references like `"ValueNoise.main:field"`
3. **Render selection**
   - top-level `[render]` chooses exact source/target
4. **Renderer**
   - `height/grayscale`
   - `scene/raytrace`

## Current Focus

- fast point-node evaluation
- field/filter execution for terrain shaping (erosion, slope, etc.)
- terrain raymarch correctness and speed
- material-node contract for shell/detail workflows

## Documentation Map

- **CLI**: current command-line workflow
- **Language**: FT node authoring model
- **Renderer**: heightfield + Whitted terrain model
- **Materials**: material-lane semantics and usage
- **Math**: scalar/vector/noise helpers

## Legacy Note

Some older docs/examples describe the previous SDF-scene-centric architecture.  
Treat those as historical context unless explicitly referenced.
