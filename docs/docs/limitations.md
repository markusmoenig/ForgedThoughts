---
sidebar_position: 6
title: Limitations
---

# Limitations

Current limitations:

- Some Forge features still fall back to the interpreter when they do not fit the current VM/JIT subset
- `subsurface` is not yet rendered as true subsurface transport
- the renderer still contains some backend-specific recursion logic
- the language is still evolving and should not be treated as stable yet

Near-term priorities:

- stabilize the Forge material runtime contract
- improve renderer correctness around custom BSDFs
- document the language and material system as they evolve
- widen VM/JIT coverage for more Forge shading and procedural code
