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

Ray tracer:

```bash
ftc ray --scene examples/glass.ft
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
- `ray` supports progressive tile updates and debug AOVs
- `path` supports adaptive sampling controls and preview overwrites
