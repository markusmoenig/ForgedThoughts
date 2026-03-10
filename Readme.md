# ForgedThoughts

ForgedThoughts is a Rust workspace for a small scene language (`.ft`) and a CPU renderer focused on signed distance field scenes.

Current state:

- FT parser, evaluator, and scene loading
- CPU SDF rendering from `.ft` files
- Fast recursive `ray` renderer for lookdev
- Progressive Monte Carlo `trace` renderer for path tracing
- Acceleration backends: `naive`, `bvh`, `bricks`
- Built-in material backends: `Lambert`, `Metal`, `Dielectric`
- FT-defined material hooks for:
  - `color`, `roughness`, `ior`, `thin_walled`
  - `emission_color`, `emission_strength`
  - `medium`, `subsurface`
  - `eval`, `pdf`, `sample`

## Workspace

- `crates/forgedthoughts`: core language + renderer library
- `crates/ftc`: CLI frontend
- `examples/`: sample `.ft` scenes and their rendered `.png` outputs

## Quickstart

Build:

```bash
cargo build
```

Validate a scene:

```bash
cargo run -p ftc -- check --scene examples/mvp.ft
```

Fast recursive ray render:

```bash
cargo run -p ftc -- ray --scene examples/glass.ft
```

Path trace:

```bash
cargo run -p ftc -- trace --scene examples/glass.ft --spp 64 --bounces 8
```

Depth render:

```bash
cargo run -p ftc -- render --scene examples/mvp.ft
```

Acceleration benchmark:

```bash
cargo run -p ftc -- bench --scene examples/mvp.ft --iterations 5 --warmup 1
```

Outputs default to the scene path with `.png` extension, so `examples/glass.ft` renders to `examples/glass.png`.

## Renderers

`ray`

- Recursive CPU renderer for quick iteration
- Progressive tiled updates
- Supports debug AOVs with `--debug-aov`
- Uses the shared material system, but still has some hardcoded reflection/refraction logic internally

`trace`

- Path tracer
- Supports adaptive controls: `--min-spp`, `--noise-threshold`
- Overwrites the destination PNG during preview updates with `--preview-every`
- This is the renderer that currently benefits most from FT-defined `eval/pdf/sample`

## Language Snapshot

FT is object-like, incremental, and scriptable:

```ft
var sphere = Sphere {
  radius: 1.0
};
sphere.pos.y = 0.3;

let mat = Dielectric {
  color: vec3(0.96, 0.99, 1.0),
  ior: 1.52,
  roughness: 0.02,
  thin_walled: 0.0
};

sphere.material = mat;
let scene = sphere;
```

Supported language pieces today include:

- `let` / `var`
- nested property assignment like `pos.x` and `rot.z`
- object literals
- scalar and `vec3` math
- material definitions with local bindings and functions

Example FT material:

```ft
material SoftGold {
  model: Metal;
  color = vec3(0.92, 0.78, 0.34);

  fn eval(ctx) {
    let ndotl = max(dot(ctx.normal, ctx.wi), 0.0);
    return mix(vec3(0.08, 0.06, 0.03), color, ndotl) * (1.0 / 3.14159265);
  }

  fn pdf(ctx) {
    return max(dot(ctx.normal, ctx.wi), 0.0) / 3.14159265;
  }

  fn sample(ctx) {
    return BsdfSample {
      wi: ctx.normal,
      f: color * (1.0 / 3.14159265),
      pdf: 1.0,
      delta: 0.0,
      apply_cos: 1.0,
      transmission: 0.0,
      thin_walled: 0.0,
      next_ior: ctx.current_ior
    };
  }
};
```

See:

- `examples/lambert.ft`
- `examples/metal.ft`
- `examples/glass.ft`
- `examples/ft_material.ft`
- `examples/ft_material_glass.ft`
- `examples/ft_material_wax.ft`
- `examples/ft_bsdf.ft`

## Material Model

There are two layers right now:

1. Built-in host BSDF backends: `Lambert`, `Metal`, `Dielectric`
2. FT-side overrides on top of those backends

An FT material can:

- set static properties like `color = vec3(1.0)`
- compute dynamic properties from hit context with `fn color(ctx) { ... }`
- define custom `eval(ctx)`, `pdf(ctx)`, and `sample(ctx)` hooks

Current hit/BSDF context includes values such as:

- `ctx.position`
- `ctx.local_position`
- `ctx.normal`
- `ctx.view_dir`
- `ctx.wo`
- `ctx.wi`
- `ctx.current_ior`
- `ctx.u1`, `ctx.u2`, `ctx.u3` for `sample(ctx)`

## Current Limits

- `subsurface` exists as material data, but true subsurface transport is not implemented yet
- `medium` currently affects transmission through simple Beer-Lambert attenuation
- FT-defined `eval/pdf/sample` are integrated in the path tracer first; the `ray` renderer still has some backend-specific recursion logic
- FT material functions are still interpreted, not VM/JIT compiled

## Development

Run checks:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Direction

The current direction is:

- keep CPU rendering practical for complex SDF scenes
- push FT materials from parameter scripting toward self-contained shareable shading code
- stabilize the FT material/runtime contract before moving to a VM and later JIT
