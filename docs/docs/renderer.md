---
sidebar_position: 4
title: Renderer
---

# Renderer Overview

The renderer is now terrain/graph oriented.

Two active outputs:

- `height/grayscale`
- `scene/raytrace` (Whitted terrain raymarch)

## Terrain Raytrace Pipeline

1. Load/evaluate TOML graph root
2. Sample graph height source (point or field)
3. Raymarch terrain heightfield
4. Compute normals/shadows/reflections
5. Shade with Whitted-style lighting model

## Field Execution

Field graphs can run through:

- CPU field path
- `wgpu` field backend (generic AST-based translation)

Current terrain path can consume GPU-rasterized field output for faster height sampling.

## Material Direction

Terrain height is the macro carrier.

Material graphs are expected to provide:

- PBR/Whitted lanes (`roughness`, `metallic`, `coat`, `transparency`, etc.)
- optional shell/displacement detail on top of terrain hit

This keeps macro terrain fast and moves micro detail power into material nodes.

## Known Tradeoff

When a field source is rasterized, it is band-limited by raster resolution.

Mitigation strategies:

- higher/adaptive field raster resolution
- hybrid macro field + live micro residual
- shell detail in material stage

## Validation Helpers

`FORGEDTHOUGHTS_REQUIRE_GPU_FIELD=1` can be used to force field graphs to fail if GPU backend translation is unavailable, avoiding silent CPU fallback during backend work.
