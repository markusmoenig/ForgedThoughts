---
sidebar_position: 3
title: CLI
---

# CLI

Validate a scene:

```bash
cargo run -p ftc -- check --scene examples/mvp.ft
```

Depth render:

```bash
cargo run -p ftc -- render --scene examples/mvp.ft
```

Fast recursive ray render:

```bash
cargo run -p ftc -- ray --scene examples/glass.ft
```

Path trace:

```bash
cargo run -p ftc -- trace --scene examples/glass.ft --spp 64 --bounces 8
```

Benchmark acceleration backends:

```bash
cargo run -p ftc -- bench --scene examples/mvp.ft --iterations 5 --warmup 1
```

Notes:

- Output defaults to the input scene path with `.png`
- `ray` supports progressive tile updates and debug AOVs
- `trace` supports adaptive sampling controls and preview overwrites
