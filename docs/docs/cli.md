---
sidebar_position: 3
title: CLI
---

# CLI

Current CLI is graph-first:

```bash
ftc <graph.toml>
```

## Basic Usage

Render a graph:

```bash
ftc examples/simple_noise.toml
```

Render terrain graph:

```bash
ftc examples/terrain_erosion.toml
```

Render a close-up material preview sphere (Substance-style):

```bash
ftc examples/terrain_erosion.toml --material-preview
```

Preview sphere with mask debug coloring:

```bash
ftc examples/terrain_erosion.toml --material-preview --material-debug mask
```

Choose an explicit output path:

```bash
ftc examples/terrain_erosion.toml --output out/terrain.png
```

Override resolution:

```bash
ftc examples/terrain_erosion.toml --width 1280 --height 720
```

Watch and rerender on save:

```bash
ftc examples/terrain_erosion.toml --watch
```

## Current Flags

- `--output <path>`
- `--width <u32>`
- `--height <u32>`
- `--tile-size <u32>`
- `--watch`
- `--material-preview`
- `--material-debug <off|mask|lanes>`
- `-v`, `-vv`

## Graph Requirements

- input must be a `.toml` graph file
- graph must include a `[render]` block
- render source must be an explicit `"type.alias:port"` reference

## Strict GPU Field Verification

For debugging field backend routing:

```bash
FORGEDTHOUGHTS_REQUIRE_GPU_FIELD=1 ftc examples/erosion_field.toml
```

This fails fast if a field graph cannot run on the GPU field backend.

## Planned Convenience Features

Planned CLI additions include:

- list available built-in nodes
- inspect node ports/metadata
- graph validation-only mode
- graph/render source introspection
