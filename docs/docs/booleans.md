---
sidebar_position: 7
title: Booleans
---

# Booleans

Forge supports a small hard-CSG core plus named SDF boolean variants inspired by `hg_sdf`.

## Parameters

- `r`
  Profile radius or blend width for rounded, chamfered, soft, pipe, and engrave-style operators.

- `n`
  Repetition count for the `*_columns` and `*_stairs` variants.

- `ra`, `rb`
  Paired profile widths for `groove` and `tongue`.

## Hard Booleans

Use operators for the basic set:

- `a + b` for union
- `a - b` for subtraction
- `a & b` for intersection

```ft
let shape = (body + cap) - cut;
let mask = body & window;
```

All examples below use the same reference setup:

- left source shape: gold metallic sphere
- right source shape: diffuse box
- shared camera, lights, environment, and checker floor
- per-operator scene source in [`examples/`](/Users/markusmoenig/ForgedThoughts/examples)

## Hard Union

`a + b`

![Hard Union](/img/booleans/boolean_union.png)

Scene: [boolean_union.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_union.ft)

## Hard Difference

`a - b`

![Hard Difference](/img/booleans/boolean_difference.png)

Scene: [boolean_difference.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_difference.ft)

## Hard Intersection

`a & b`

![Hard Intersection](/img/booleans/boolean_intersection.png)

Scene: [boolean_intersection.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_intersection.ft)

## Union Round

`a.union_round(b, r)`

![Union Round](/img/booleans/boolean_union_round.png)

Scene: [boolean_union_round.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_union_round.ft)

## Union Chamfer

`a.union_chamfer(b, r)`

![Union Chamfer](/img/booleans/boolean_union_chamfer.png)

Scene: [boolean_union_chamfer.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_union_chamfer.ft)

## Union Columns

`a.union_columns(b, r, n)`

Scene: [boolean_union_columns.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_union_columns.ft)

Image omitted for now while the operator is being tuned.

## Union Stairs

`a.union_stairs(b, r, n)`

![Union Stairs](/img/booleans/boolean_union_stairs.png)

Scene: [boolean_union_stairs.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_union_stairs.ft)

## Union Soft

`a.union_soft(b, r)`

![Union Soft](/img/booleans/boolean_union_soft.png)

Scene: [boolean_union_soft.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_union_soft.ft)

## Intersect Round

`a.intersect_round(b, r)`

![Intersect Round](/img/booleans/boolean_intersect_round.png)

Scene: [boolean_intersect_round.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_intersect_round.ft)

## Intersect Chamfer

`a.intersect_chamfer(b, r)`

![Intersect Chamfer](/img/booleans/boolean_intersect_chamfer.png)

Scene: [boolean_intersect_chamfer.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_intersect_chamfer.ft)

## Intersect Columns

`a.intersect_columns(b, r, n)`

Scene: [boolean_intersect_columns.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_intersect_columns.ft)

Image omitted for now while the operator is being tuned.

## Intersect Stairs

`a.intersect_stairs(b, r, n)`

![Intersect Stairs](/img/booleans/boolean_intersect_stairs.png)

Scene: [boolean_intersect_stairs.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_intersect_stairs.ft)

## Difference Round

`a.diff_round(b, r)`

![Difference Round](/img/booleans/boolean_diff_round.png)

Scene: [boolean_diff_round.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_diff_round.ft)

## Difference Chamfer

`a.diff_chamfer(b, r)`

![Difference Chamfer](/img/booleans/boolean_diff_chamfer.png)

Scene: [boolean_diff_chamfer.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_diff_chamfer.ft)

## Difference Columns

`a.diff_columns(b, r, n)`

Scene: [boolean_diff_columns.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_diff_columns.ft)

Image omitted for now while the operator is being tuned.

## Difference Stairs

`a.diff_stairs(b, r, n)`

![Difference Stairs](/img/booleans/boolean_diff_stairs.png)

Scene: [boolean_diff_stairs.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_diff_stairs.ft)

## Pipe

`a.pipe(b, r)`

Scene: [boolean_pipe.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_pipe.ft)

Image omitted for now while the operator is being tuned.

## Engrave

`a.engrave(b, r)`

Scene: [boolean_engrave.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_engrave.ft)

Image omitted for now while the operator is being tuned.

## Groove

`a.groove(b, ra, rb)`

Scene: [boolean_groove.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_groove.ft)

Image omitted for now while the operator is being tuned.

## Tongue

`a.tongue(b, ra, rb)`

Scene: [boolean_tongue.ft](/Users/markusmoenig/ForgedThoughts/examples/boolean_tongue.ft)

Image omitted for now while the operator is being tuned.
