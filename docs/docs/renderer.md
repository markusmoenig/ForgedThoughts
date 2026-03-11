---
sidebar_position: 4
title: Renderer
---

# Renderer

ForgedThoughts is built around a CPU-first SDF renderer. The main production path today is `trace`, a classical Whitted-style recursive renderer tuned for fast iteration on signed distance field scenes.

## Why Whitted

The project deliberately prioritizes:

- fast feedback while modeling
- predictable render cost on the CPU
- clear reflections and refractions for SDF scenes
- practical lookdev over slow global-illumination convergence

That is why the current renderer is not centered on a path tracer. For this project, a CPU path tracer quickly becomes too slow and too noisy to be the default tool.

## Current Render Paths

- `trace`
  The main renderer. It handles direct lighting, recursive reflection, recursive transmission, medium attenuation, soft-shadow-capable sphere lights, environment backgrounds, and debug AOVs.

- `depth`
  A fast grayscale depth preview for quick shape iteration and scene inspection.

## Current Trace Features

- Whitted-style recursive reflections
- dielectric transmission and refraction
- rough dielectric approximation with deterministic multi-sample branching
- Beer-Lambert medium attenuation
- smooth-boolean shading/material blending
- point lights
- sphere lights for softer shadows
- environment lights and procedural environment backgrounds
- debug AOVs such as depth, normal, material id, IOR, transmission, Fresnel, and hit distance

## Important Tradeoffs

- `trace` is not a full global-illumination renderer
- rough glass is approximate and budgeted to stay practical on the CPU
- some material behavior is still renderer-specific rather than fully generic
- renderer correctness is improving incrementally, but speed remains a core design constraint

## Direction

The renderer direction is incremental improvement of `trace`, not replacing it with a slow unbiased architecture.

Near-term goals:

- better dielectric quality
- better light types and softer shadow control
- stronger environment lighting and reflections
- more GI-like features where they fit the CPU/Whitted model
- continued regression coverage for stable scenes
