# ForgedThoughts

ForgedThoughts is a Rust workspace for a small scene language (`.ft`) and a CPU renderer focused on signed distance field scenes.

Current state:

- Forge parser, evaluator, and scene loading
- Fast depth preview rendering from `.ft` files
- Classical Whitted-style CPU rendering for lookdev
- Acceleration backends: `naive`, `bvh`, `bricks`
- Built-in lights: `PointLight`, `SphereLight`, `EnvLight`
- Built-in material backends: `Lambert`, `Metal`, `Dielectric`
- Forge-defined material hooks for:
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
ftc check --scene examples/mvp.ft
```

Trace renderer:

```bash
ftc --scene examples/glass.ft
```

Trace renderer with supersampling:

```bash
ftc --scene examples/glass.ft --aa 4
```

Depth preview:

```bash
ftc depth --scene examples/mvp.ft
```

Depth preview with smoother edges:

```bash
ftc depth --scene examples/mvp.ft --aa 4
```

Watch and re-render on save:

```bash
ftc depth --scene examples/mvp.ft --watch
```

Acceleration benchmark:

```bash
ftc bench --scene examples/mvp.ft --iterations 5 --warmup 1
```

Outputs default to the scene path with `.png` extension, so `examples/glass.ft` renders to `examples/glass.png`.

## Renderers

Main renderer

- Classical Whitted-style CPU renderer for quick iteration
- Progressive tiled updates
- Supports `--aa` for camera supersampling
- Supports debug AOVs with `--debug-aov`
- Uses the shared material system, but still has some hardcoded reflection/refraction logic internally

`depth`

- Fast depth preview renderer
- Intended for quick shape iteration
- Supports `--aa` for smoother depth edges
- Supports `--watch` for iterative modeling loops

## Language Snapshot

Forge is object-like, incremental, and scriptable:

```ft
var sphere = Sphere {
  radius: 1.0
};
sphere.pos.y = 0.3;

let mat = Dielectric {
  color: #f5fcff,
  ior: 1.52,
  roughness: 0.02,
  thin_walled: 0.0
};

sphere.material = mat;
let scene = sphere;
```

Supported language pieces today include:

- top-level functions
- top-level imports
- top-level exports
- `let` / `var`
- nested property assignment like `pos.x` and `rot.z`
- object literals
- scalar and `vec3` math
- hex color literals like `#ff0000` and `#f00`
- material definitions with local bindings and functions
- environment definitions with local bindings and functions
- custom SDF definitions with `sdf Name { fn distance(p) { ... } }`
- hard booleans with `+`, `-`, and `&`
- named `hg_sdf`-style boolean variants like `union_round`, `diff_chamfer`, and `intersect_stairs`

Example Forge material:

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

Example top-level helper functions:

```ft
fn accent() {
  return #ebc757;
}

fn tint(base, amount) {
  return mix(base, vec3(1.0), amount);
}

fn make_gold() {
  return Metal {
    color: tint(accent(), 0.12),
    roughness: 0.18
  };
}
```

See:

- `examples/lambert.ft`
- `examples/metal.ft`
- `examples/glass.ft`
- `examples/ft_sdf.ft`
- `examples/imports.ft`
- `examples/ft_material.ft`
- `examples/ft_material_glass.ft`
- `examples/ft_material_wax.ft`
- `examples/ft_bsdf.ft`

Example custom SDF:

```ft
sdf SoftBlob {
  let wave_scale = 0.16;

  fn bounds() {
    return vec3(1.2, 1.2, 1.1);
  }

  fn warp(p) {
    return vec3(p.x, p.y + sin(p.x * 4.0) * wave_scale, p.z);
  }

  fn distance(p) {
    let q = warp(p);
    return length(q) - 1.0;
  }
};

let scene = SoftBlob {};
```

`fn bounds()` is optional, but it matters for performance. Without it, custom SDFs fall back to a very conservative bound and acceleration quality drops sharply.

Example procedural environment:

```ft
environment Sky {
  let zenith = #4d74c7;
  let horizon = #d8e7ff;

  fn color(dir) {
    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(horizon, zenith, t);
  }
};
```

`color(dir)` is used as the visible background on misses in the main renderer and in `depth`.

## Imports

Forge supports top-level imports:

```ft
import "./shared/materials.ft";
import "Gold" as gold;
import "SoftBlob" as blob;
import "Studio";
```

Import rules:

- `./...` and `../...` resolve relative to the current file on disk
- `materials/...`, `objects/...`, and `scenes/...` resolve from the embedded built-in library
- bare built-in names like `Glass`, `SoftBlob`, and `Studio` also resolve from the embedded built-in library
- `as name` namespaces the imported top-level symbols under `name.`
- each import is loaded only once
- cyclic imports are rejected

Files can also declare explicit exports:

```ft
let private_color = #ebc757;
material Gold {
  model: Metal;
  color = private_color;
  roughness = 0.18;
};

export { Gold };
```

List the built-in library from the CLI:

```bash
ftc list materials
ftc list objects
ftc list scenes
```

Each entry includes its path, a short description, and semantic tags.

## Material Model

There are two layers right now:

1. Built-in host BSDF backends: `Lambert`, `Metal`, `Dielectric`
2. Forge-side overrides on top of those backends

A Forge material can:

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
- Forge-defined `eval/pdf/sample` are currently most useful through the shared material system, but the renderer still has some backend-specific recursion logic
- Forge material functions are still interpreted, not VM/JIT compiled

## Development

Run checks:

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Direction

The current direction is:

- keep CPU rendering practical for complex SDF scenes
- push Forge materials from parameter scripting toward self-contained shareable shading code
- stabilize the Forge material/runtime contract before moving to a VM and later JIT
