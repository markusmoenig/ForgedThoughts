---
sidebar_position: 6
title: Limitations
---

# Limitations

Current limitations:

- Forge material functions are interpreted, not VM/JIT compiled
- `subsurface` is not yet rendered as true subsurface transport
- the `ray` renderer still contains backend-specific recursion logic
- the path tracer is the more correct target for custom BSDF work
- the language is still evolving and should not be treated as stable yet

Near-term priorities:

- stabilize the Forge material runtime contract
- improve renderer correctness around custom BSDFs
- document the language and material system as they evolve
- move hot Forge shading code toward a VM and then JIT
