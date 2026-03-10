---
sidebar_position: 2
title: Install
---

# Install

## Prerequisites

- Rust toolchain with `cargo`

## Install `ftc`

The CLI will be distributed as a Cargo package.

```bash
cargo install ftc
```

Until that is published, build and run from source:

```bash
cargo build
cargo run -p ftc -- --help
```

## Validate the setup

Check that the CLI is available:

```bash
ftc --help
```

Or from source:

```bash
cargo run -p ftc -- --help
```

## Build from source

```bash
cargo test
```

If you want to work on ForgedThoughts itself instead of only using the CLI, clone the repo and build it locally.
