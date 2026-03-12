---
sidebar_position: 5
title: Objects
---

# Objects

Forge scenes are built from object literals, transforms, CSG composition, and custom SDF definitions.

## Built-in Primitives

Current built-ins include:

- `Sphere`
- `Box`
- `Cylinder`
- `Torus`
- `ExtrudePolygon`
- `Room`

Custom Forge SDF assets can also be parameterized and instantiated with per-instance overrides, just like materials:

```forge
import "SoftBlob";

let blob = SoftBlob {
  radius: 1.4,
  warp_frequency: 6.0,
  warp_amount: 0.22
};
```

The same mechanism also works for more semantic library objects:

```forge
import "Cupboard";
import "Lamp";
import "Table";

let cupboard = Cupboard {
  width: 1.8,
  height: 2.2,
  depth: 0.65,
  open_amount: 0.35
};

let table = Table {
  width: 1.8,
  depth: 0.9,
  height: 0.76,
  top_thickness: 0.08,
  leg_radius: 0.05
};

let lamp = Lamp {
  shade_material: Lambert { color: #efe2cf },
  bulb_material: Lambert {
    color: #fff3dd,
    emission_color: #fff1d6,
    emission_strength: 5.0
  }
};
```

Custom object assets can also define their own default anchors inside the asset itself, so instances inherit meaningful placement points like `TopSurface` or `FrontCenter` automatically. `Table` is a good early adjective-style asset because it exposes obvious shape parameters like width, depth, height, tabletop thickness, and leg radius. Lowered semantic assets can also expose named part material slots like `top_material`, `leg_material`, `door_material`, or `bulb_material`.

Forge also supports part-oriented assignment syntax for these semantic assets:

```forge
table.top.material = Lambert { color: #7a4c35 };
table.legs.material = Metal { color: #2b3138, roughness: 0.22 };

cupboard.door.material = Lambert { color: #d7c4a8 };
lamp.bulb.material = Lambert {
  color: #fff3dd,
  emission_color: #fff1d6,
  emission_strength: 5.0
};
```

Today this path is focused on material assignment for named semantic parts. It is the first slice of future part access like `table.legs.material = ...` and later deeper part editing.

The same named parts can also be used as layout targets, which keeps scene code semantic instead of forcing manual offsets from whole-object pivots.

For example, a room scene can place both a cupboard and table into a corner, then attach another object to the table's own anchor:

```forge
import "Cupboard";
import "Lamp";
import "Table";

let room = Room {
  width: 8.0,
  height: 4.0,
  depth: 8.0,
  wall_thickness: 0.18
};

var cupboard = Cupboard {
  width: 1.8,
  height: 2.2,
  depth: 0.62,
  open_amount: 0.2
}
  .attach(room, BackRightCorner)
  .offset_x(-0.12)
  .offset_z(0.12);

var table = Table {
  width: 1.7,
  depth: 0.9,
  height: 0.78
}
  .attach(room, BackRightCorner)
  .offset_x(-2.1)
  .offset_z(1.35);

var lamp = Lamp {
  bulb_material: Lambert {
    color: #fff3dd,
    emission_color: #fff1d6,
    emission_strength: 5.0
  }
}
  .attach(cupboard.body, Top, Bottom);

var vase = Sphere {
  radius: 0.18
}
  .attach(table.top, Top)
  .offset_x(-0.35);
```

Supported fields today:

`Sphere`

- `radius` or `r`
- `pos.x`, `pos.y`, `pos.z` or legacy `x`, `y`, `z`
- `rot.x`, `rot.y`, `rot.z` or legacy `rot_x`, `rot_y`, `rot_z`
- `material`

```forge
let ball = Sphere {
  radius: 1.0,
  material: Metal {
    color: #ebc757,
    roughness: 0.18
  }
};
```

`Box`

- `size: vec3(...)`
- `pos.*`
- `rot.*`
- `material`

```forge
let block = Box {
  size: vec3(1.2, 0.8, 1.2)
};
```

`Cylinder`

- `radius` or `r`
- `height` or `h`
- `pos.*`
- `rot.*`
- `material`

```forge
let column = Cylinder {
  radius: 0.5,
  height: 2.4
};
```

`Torus`

- `major_radius` or `R`
- `minor_radius` or `r`
- `pos.*`
- `rot.*`
- `material`

```forge
let ring = Torus {
  major_radius: 1.0,
  minor_radius: 0.2
};
```

`ExtrudePolygon`

- `sides` or `n` with a minimum of `3`
- `radius` or `r`
- `height` or `h`
- `pos.*`
- `rot.*`
- `material`

This is a regular N-gon extruded along the Y axis.

```forge
let hex = ExtrudePolygon {
  sides: 6,
  radius: 0.8,
  height: 0.35
};
```

`Room`

- `width`
- `height`
- `depth`
- `wall_thickness`
- `floor_material`
- `wall_material`
- `back_wall_material`
- `front_wall_material`
- `left_wall_material`
- `right_wall_material`
- `ceiling_material`
- `show_floor`
- `show_back_wall`
- `show_front_wall`
- `show_left_wall`
- `show_right_wall`
- `show_ceiling`
- `pos.*`
- `rot.*`

`Room` is a semantic built-in object that expands to a floor and optional walls/ceiling with separate material slots.

```forge
let room = Room {
  width: 8.0,
  height: 4.0,
  depth: 8.0,
  wall_thickness: 0.18,
  floor_material: CheckerFloor {},
  wall_material: Lambert { color: #f2efe8 },
  show_back_wall: 1.0,
  show_right_wall: 1.0,
  show_front_wall: 0.0,
  show_left_wall: 0.0,
  show_ceiling: 0.0
};
```

Example:

```forge
var sphere = Sphere {
  radius: 1.0
};

sphere.pos.y = 0.6;
sphere.rot.z = 12.0;

let scene = sphere;
```

Transforms are currently driven with nested properties like `pos.x`, `pos.y`, `rot.x`, and `rot.z`. For relational placement like “on top of floor” or “right of sphere”, see the layout section in [Language](./language.md#layout).

Rounding is not yet a native primitive field like `Box { rounding: 0.2 }`. Right now rounded or beveled shapes are created with shape operators such as `.round(r)`.

## Shape Operators

For simple profile changes on a single shape, Forge supports:

- `shape.round(r)` for rounded/beveled forms

For boolean composition, see the dedicated [Booleans](./booleans.md) page.

## Custom SDFs

For geometry that goes beyond the built-ins, define a Forge SDF with a single distance contract:

```forge
sdf SoftBlob {
  fn bounds() {
    return vec3(1.2, 1.2, 1.1);
  }

  fn warp(p) {
    return vec3(p.x, p.y + sin(p.x * 4.0) * 0.16, p.z);
  }

  fn distance(p) {
    let q = warp(p);
    return length(q) - 1.0;
  }
};

let scene = SoftBlob {};
```

Rules:

- `fn distance(p)` is required
- `p` is evaluated in local/object space
- helper functions can be reused inside the SDF block
- `fn bounds()` is optional but strongly recommended

## Why `bounds()` Matters

Without `bounds()`, custom SDFs fall back to a very conservative bound. That keeps rendering correct, but acceleration gets much worse and scenes can become noticeably slower.

Current `bounds()` behavior:

- it returns a local half-extent as `vec3(...)`, or a numeric radius
- the renderer currently turns that into a conservative bounding sphere radius
- that is already much better than the old giant fallback bound

So for most custom shapes, add `bounds()` early even if it is only approximate.

## Current Limits

- custom SDFs currently expose `distance(p)` and optional `bounds()`
- exact rotated bounds are not computed yet; the current implementation uses a conservative radius from `bounds()`
- custom SDF distance code now uses the VM/JIT path for the supported numeric and vec3 subset, with interpreter fallback for the rest

## Imports and Reuse

Objects and SDFs can be shared through imports:

```forge
import "./shared/blob.ft";
import "SoftBlob" as blob;
```

Use relative imports for project-local assets and built-in library imports for reusable bundled definitions.
