# ForgedThoughts

ForgedThoughts is a Rust workspace for terrain-first node graphs:

- FT node definitions (`node ... { fn eval(ctx) { ... } }`)
- TOML graph instancing/wiring (`[Type.alias]`)
- Height/field rendering and Whitted terrain raymarching

The current primary authoring path is **TOML graphs + FT nodes**.

## Workspace

- `crates/forgedthoughts`: language, graph loader, evaluator, render backends
- `crates/ftc`: CLI frontend
- `examples/`: graph examples and rendered outputs

## Quickstart

Build:

```bash
cargo build
```

Render a graph:

```bash
ftc examples/simple_noise.toml
```

Render terrain with erosion:

```bash
ftc examples/terrain_erosion.toml
```

Render to custom output/resolution:

```bash
ftc examples/terrain_erosion.toml --output out/terrain.png --width 1280 --height 720
```

Watch and rerender:

```bash
ftc examples/terrain_erosion.toml --watch
```

## Current Architecture

1. FT defines reusable node behavior.
2. TOML graph files instantiate and connect nodes.
3. `[render]` selects an exact graph source and target.
4. Terrain raymarch uses graph-driven height/field outputs.

Current render targets:

- `height/grayscale`
- `scene/raytrace`

## Performance Model

- Point nodes: JIT-specialized scalar sampling
- Field nodes: pass-style execution (CPU path + `wgpu` field backend)
- Terrain path: Whitted raymarch over graph-provided height source

## Material Direction

Material nodes are the main detail layer on top of macro terrain.

The intended pipeline:

1. Heightfield hit + normal (macro geometry)
2. Material evaluation at hit context
3. Shell/displacement detail phase
4. Whitted shading lanes (`albedo`, `roughness`, `metallic`, `coat`, `transparency`, etc.)

See:

- [MaterialNodeContract.md](/Users/markusmoenig/ForgedThoughts/MaterialNodeContract.md)
- [NodeContextDesign.md](/Users/markusmoenig/ForgedThoughts/NodeContextDesign.md)
- [GraphDesign.md](/Users/markusmoenig/ForgedThoughts/GraphDesign.md)

## Planned CLI Convenience

The CLI is intentionally minimal today, but expected convenience features include:

- list available built-in nodes
- inspect node input/output ports
- validate graph connections/types without rendering
- render-source introspection for graph debugging

## Legacy Note

Older SDF-scene-centric docs/examples are legacy context.

The active direction is terrain/node graph workflows, not expanding the old scene authoring architecture.
