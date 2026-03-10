---
sidebar_position: 1
title: Overview
---

# ForgedThoughts

ForgedThoughts is a CPU-first rendering project built around:

- a small scene language called `FT`
- signed distance field scene construction
- fast recursive lookdev rendering
- progressive path tracing
- programmable materials that are moving from host-defined parameters toward FT-defined shading logic

Today the project is already usable for:

- authoring `.ft` scene files
- rendering SDF scenes with `ray`, `trace`, or `render`
- rendering SDF scenes with `ray`, `path`, or `render`
- testing material ideas with `Lambert`, `Metal`, and `Dielectric`
- writing FT material hooks such as `color`, `roughness`, `ior`, `medium`, and custom `eval/pdf/sample`

The long-term direction is to make materials and later more of the rendering logic programmable in FT, then move hot paths to a VM and JIT.
