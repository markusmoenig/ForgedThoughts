---
sidebar_position: 3
title: CLI
---

# CLI

List built-in library assets:

```bash
ftc list materials
ftc list objects
ftc list scenes
```

Validate a scene:

```bash
ftc check --scene examples/mvp.ft
```

Shaded preview render:

```bash
ftc render --scene examples/mvp.ft
```

Higher-quality preview edges:

```bash
ftc render --scene examples/mvp.ft --aa 4
```

Watch and re-render on save:

```bash
ftc render --scene examples/mvp.ft --watch
```

Ray tracer:

```bash
ftc ray --scene examples/glass.ft
```

Ray tracer with supersampling:

```bash
ftc ray --scene examples/glass.ft --aa 4
```

Path trace:

```bash
ftc path --scene examples/glass.ft --spp 64 --bounces 8
```

Benchmark acceleration backends:

```bash
ftc bench --scene examples/mvp.ft --iterations 5 --warmup 1
```

Notes:

- Output defaults to the input scene path with `.png`
- `render` is a fast shaded preview path with materials but without shadow tracing
- `render` and `ray` support `--aa` for camera supersampling
- `ray` supports progressive tile updates and debug AOVs
- `path` supports adaptive sampling controls and preview overwrites
- `check`, `render`, `ray`, and `path` support `--watch` to rerun when the scene file changes
- current `--watch` tracks the scene file itself, not imported files yet
