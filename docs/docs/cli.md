---
sidebar_position: 3
title: CLI
---

# CLI

List built-in library assets with descriptions and tags:

```bash
ftc list materials
ftc list objects
ftc list scenes
```

Validate a scene:

```bash
ftc check --scene examples/mvp.ft
```

Depth preview:

```bash
ftc depth --scene examples/mvp.ft
```

Smoother depth edges:

```bash
ftc depth --scene examples/mvp.ft --aa 4
```

Watch and rerun on save:

```bash
ftc depth --scene examples/mvp.ft --watch
```

Default trace renderer:

```bash
ftc --scene examples/glass.ft
```

Default trace renderer with supersampling:

```bash
ftc --scene examples/glass.ft --aa 4
```

Benchmark acceleration backends:

```bash
ftc bench --scene examples/mvp.ft --iterations 5 --warmup 1
```

Notes:

- Output defaults to the input scene path with `.png`
- `ftc` without a subcommand runs the trace renderer
- `depth` is a fast depth preview for shape iteration
- `depth` and the default trace path support `--aa` for camera supersampling
- `trace` supports progressive tile updates and debug AOVs
- `check`, `depth`, and the default trace path support `--watch` to rerun when the scene file changes
- current `--watch` tracks the scene file itself, not imported files yet
