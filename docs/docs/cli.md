---
sidebar_position: 3
title: CLI
---

# CLI

Render a TOML graph:

```bash
ftc examples/simple_noise.toml
```

Choose an explicit output path:

```bash
ftc examples/simple_noise.toml --output out/noise.png
```

Change render resolution:

```bash
ftc examples/simple_noise.toml --width 1024 --height 1024
```

Change the sampled world-space range:

```bash
ftc examples/simple_noise.toml --world-size 4.0
```

Watch and rerender on save:

```bash
ftc examples/simple_noise.toml --watch
```

Notes:

- Input must be a TOML graph file
- Output defaults to the input path with `.png`
- `--watch` currently tracks the graph file itself, not imported files yet
