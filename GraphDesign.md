# Graph Design

This document defines the first concrete graph format for the terrain/node system.

The purpose of the graph file is:

- instantiate reusable FT node definitions
- assign constant parameter values
- connect node outputs to node inputs
- define what exactly should be rendered from the graph

The graph file is not FT source.

FT remains the language for reusable node definitions.

The graph file is a separate TOML-based graph description.

## Design Goals

The graph format should be:

1. Human-readable
2. Diff-friendly
3. Easy to validate
4. Easy to generate from a future UI editor
5. Explicit about render targets

## File Format

Graphs use TOML.

Example:

```toml
version = 1

[render]
stage = "height"
target = "grayscale"
source = "noise.main:field"

[noise.main]
scale = 0.13
seed = 31
wrap = true
pos = [30, 213]
```

## Node Instances

Node instances are declared as:

```toml
[type.alias]
```

Examples:

```toml
[noise.main]
[levels.main]
[mask_blend.main]
[import_layer.2]
```

Meaning:

- `type` is the node definition name or node-kind selector
- `alias` is the unique instance id for that type in the graph

Aliasing is required.

It is how you place multiple instances of the same node type in one graph.

Examples:

```toml
[Multiply.low]
[Multiply.high]
```

These are two separate `Multiply` nodes with different parameters or connections.

The full instance id is:

```text
type.alias
```

Examples:

- `noise.main`
- `levels.main`
- `import_layer.2`

## Why No `node.` Prefix

The whole file is already a graph file.

So a prefix like:

```toml
[node.noise.main]
```

adds syntax but no real information.

The shorter form:

```toml
[noise.main]
```

is clearer and easier to read.

## Connection References

Graph links use a shared string reference syntax:

```text
type.alias:port
```

Examples:

- `"noise.main:field"`
- `"levels.main:field"`
- `"rock_mat.main:albedo"`
- `"terrain.main:height"`

Meaning:

- left side identifies a node instance
- right side identifies an output port

This same syntax is used:

- inside node input assignments
- inside the `[render]` block

## Port Kinds

Ports are not just named outputs. They also have execution semantics.

Current direction:

- `point_scalar`
- `field_scalar`

Meaning:

- `point_scalar`: a live point-evaluated scalar kernel, sampled on demand at `(x, z)` or via `eval(ctx)`
- `field_scalar`: a scalar field produced by a pass/tile execution stage

Current built-in nodes are `point_scalar`.

That means the current graph runtime stays fully live and procedural.

The important distinction is:

- `point_scalar` is evaluated at sample time
- `field_scalar` is a field result that may come from tiled or cached execution

This does not imply “export everything to a low-res texture”.

A `field_scalar` may still be:

- generated at high resolution
- generated per tile
- regenerated on demand
- cached adaptively

The graph type system should track this distinction explicitly so pass nodes like erosion do not get forced into the point-sampled `eval(ctx)` model.

## Constants vs Connections

In node instance tables:

- numeric values are constants
- booleans are constants
- arrays are constants
- strings may be either plain strings or connection refs
- editor-only fields like `pos` are metadata, not runtime ports

Examples:

```toml
[noise.main]
scale = 0.13
seed = 31
wrap = true
pos = [30, 213]
```

```toml
[levels.main]
in = "noise.main:field"
level = 0.48
width = 0.42
pos = [209, 212]
```

In the first implementation, the loader should detect connection refs using the exact reference grammar.

If a string does not match the reference grammar, it is treated as a literal string.

Example:

```toml
[import_layer.main]
source = "stones"
```

Here `source` is a literal asset/source name, not a graph connection.

## Material Instance Overrides

Material eligibility controls should default inside the material node and be overrideable per graph instance.

Typical override fields:

- `height`, `height_band`
- `slope`, `slope_band`
- `mask_influence`
- `blend_softness`

These are ordinary instance parameters in `[type.alias]` tables, not a separate override mechanism.

Example:

```toml
[RockMaterial.cliff]
height = 0.6
height_band = 0.8
slope = 0.75
slope_band = 0.5
mask_influence = 0.4
blend_softness = 0.2
```

## Render Block

The graph defines render intent through a top-level `[render]` block.

This is required.

The render block solves:

- rendering a scalar field as grayscale
- rendering a color output as a texture
- rendering a specific material output when many materials exist in the graph
- later rendering terrain + material combinations in 3D

## Render Fields

The render block contains:

- stage
- target
- one or more exact source bindings

Examples:

### Scalar Preview

```toml
[render]
stage = "height"
target = "grayscale"
source = "noise.main:field"
```

### Color Texture Preview

```toml
[render]
stage = "surface"
target = "texture"
color = "mask_blend.main:color"
```

### Material Texture Output

```toml
[render]
stage = "material"
target = "texture"
color = "rock_mat.main:albedo"
roughness = "rock_mat.main:roughness"
metallic = "rock_mat.main:metallic"
coat = "rock_mat.main:coat"
coat_roughness = "rock_mat.main:coat_roughness"
transparency = "water_mat.main:transparency"
ior = "water_mat.main:ior"
height = "terrain_mix.main:height"
normal = "terrain_mix.main:normal"
```

### Future Scene Render

```toml
[render]
stage = "scene"
target = "raytrace"
height = "terrain.main:height"
material = "terrain_mat.main:material"
```

For shell-enabled materials in `scene/raytrace`:

- terrain height source provides macro hit
- material node provides shell controls (`shell_height`, `shell_mask`, etc.)
- final shading is evaluated at shell hit, not macro hit

## Important Rule

The render block must allow exact graph sources.

No implicit guessing like:

- “main material”
- “last output”
- “default color”

For Whitted-oriented material graphs, render bindings should explicitly provide:

- `color` (albedo)
- `roughness`
- `metallic`
- `coat`
- `coat_roughness`
- `transparency`

Optional material bindings:

- `ior`
- `emission`
- `shell_height`
- `shell_mask`

## V1 Port Schema

The current implementation validates a small schema-backed node set:

- `ValueNoise`
- `Constant`
- `Add`
- `Multiply`

Current port rules:

- `ValueNoise` outputs `field: point_scalar`
- `Constant` outputs `field: point_scalar`
- `Add` inputs `a`, `b` and outputs `field: point_scalar`
- `Multiply` input `input` and output `field: point_scalar`

Current param rules:

- `ValueNoise`: `scale`, `octaves`, `lacunarity`, `persistence`
- `Constant`: `value`
- `Multiply`: `factor`

Current editor metadata:

- `pos`

Unknown node types are not schema-valid graph nodes yet.

That is a temporary implementation limit until FT node definitions expose graph-port metadata directly.

If the user wants a specific material or output, they must name the exact source:

```toml
color = "rock_mat.main:albedo"
```

This is especially important once several material-producing subgraphs exist in the same graph.

## Editor Metadata

Fields like:

```toml
pos = [30, 213]
```

are editor metadata, not runtime semantics.

For V1 they can live inline in the node table for simplicity.

Later they may move to a dedicated UI subtable if needed:

```toml
[noise.main.ui]
pos = [30, 213]
```

For now, inline `pos` is acceptable.

## Example Graph

This is the current preferred style:

```toml
version = 1

[render]
stage = "material"
target = "texture"
color = "mask_blend.main:color"
height = "add.main:field"
roughness = 0.9
metallic = 0.0
coat = 0.2
coat_roughness = 0.15
transparency = 0.0
ior = 1.5
emission = 0.0

[levels.main]
in = "noise.main:field"
level = 0.48
width = 0.42
pos = [209, 212]

[import_layer.main]
pos = [33, 32]
source = "stones"

[import_layer.2]
input_1 = "levels.main:field"
pos = [371, 179]
source = "soil_overlay"

[mask_blend.main]
a = "import_layer.main:color"
b = "import_layer.2:color"
factor = 1.0
mask = "levels.main:field"
pos = [637, 54]

[noise.main]
pos = [30, 213]
scale = 0.13
seed = 31
wrap = true

[add.main]
a = "import_layer.main:height"
b = "import_layer.2:height"
pos = [651, 274]
```

## Validation Rules

The graph loader should validate:

1. Every table name matches `type.alias`
2. Every referenced `type.alias:port` exists
3. Ports are valid for the referenced node type
4. Input types match output types
5. The graph is acyclic for point-node execution
6. The `[render]` block exists
7. `[render]` bindings are valid for the selected `stage` and `target`

## Node Definitions And Ports

This graph format depends on node definitions declaring:

- category
- stage compatibility
- input ports
- output ports
- parameter defaults and types

The graph file should not redefine port types.

It only:

- instantiates
- assigns
- connects
- selects render sources

## V1 Constraints

The first implementation should stay narrow:

1. only point-evaluated height nodes
2. only scalar outputs
3. only one stage: `height`
4. one main render target: `grayscale`
5. connection refs use the shared `"type.alias:port"` format

That is enough to replace the current FT graph example with a real graph file.

## Neighborhood And Pass Nodes

Some future nodes, especially erosion and other filters, are not point-evaluated nodes.

Those nodes will need:

- neighborhood access
- tiles or buffers
- sometimes multiple passes

So the graph system must eventually support more than one execution class:

1. point nodes
2. pass/tile nodes

For V1, ignore pass nodes.

They should be added only after the point-node graph system is stable.

## Suggested First Deliverables

1. A `simple_noise.toml` example using this format
2. A graph loader/validator for:
   - instance ids
   - connection refs
   - render block
3. A minimal lowering path from graph instance values to the current `eval(ctx)` height kernel

## Recommended Direction

Use this format as the first real graph representation:

- node instances: `[type.alias]`
- connections: `"type.alias:port"`
- render selection: `[render]`

This is concrete enough for implementation and simple enough to evolve.
