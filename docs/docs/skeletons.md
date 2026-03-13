---
sidebar_position: 5
title: Skeletons
---

# Skeletons

Skeletons are semantic assets for joints and named rigid segments. They are not renderable geometry by themselves, but they provide structured attachment targets for robot and humanoid assemblies.

## Definitions

Forge skeletons use explicit `joint`, `bone`, and optional `chain` declarations:

```forge
skeleton Robot {
  joint pelvis = vec3(0.0, 1.0, 0.0);
  joint neck = vec3(0.0, 1.6, 0.0);
  joint shoulder_l = vec3(-0.28, 1.55, 0.0);
  joint elbow_l = vec3(-0.60, 1.30, 0.0);
  joint hand_l = vec3(-0.85, 1.05, 0.0);

  bone torso = pelvis, neck;
  bone forearm_l = elbow_l, hand_l;
  chain arm_l = shoulder_l, elbow_l, hand_l;
};
```

For real rigs, a `chain` should describe a fixed-length two-bone segment:

```forge
chain leg_l = hip_l, knee_l, foot_l;
chain arm_r = shoulder_r, elbow_r, hand_r;
```

Use them like any other asset:

```forge
import "Robot";

let rig = Robot {
  height: 1.9
};
```

## IK Targets

Skeleton instances can define local IK targets through an `ik` object. Targets are keyed by chain name or end-joint name and keep the segment lengths fixed:

```forge
let rig = Robot {
  height: 1.85,
  ik: {
    leg_l: vec3(-0.26, -0.925, 0.10),
    leg_r: vec3(0.21, -0.90, -0.05),
    arm_l: vec3(-0.56, 0.02, 0.20),
    arm_r: vec3(0.48, 0.10, 0.08)
  }
}
  .attach(stone, Top + 0.08);
```

This is the first kinematics slice:

- two-bone chains only
- targets are local to the skeleton instance
- segment lengths stay fixed
- bound parts follow automatically

## Joints And Bones

Skeleton instances expose:

- joints as named anchors such as `"hand_l"` or `rig.hand_l`
- bones as semantic part proxies such as `rig.forearm_l`

That makes them usable with the normal layout system:

```forge
let hand_marker = Sphere { radius: 0.08 }
  .attach(rig.hand_l, Center);
```

## Binding Parts

`bind(...)` is the rigid placement helper for bone-shaped parts.

```forge
import "Robot";
import "RobotSegment";

let rig = Robot {
  height: 1.9
}
  .attach(floor, Top);

let forearm = RobotSegment {
  width: 0.14,
  depth: 0.14,
  length: 0.48,
  round: 0.03
}
  .bind(rig.forearm_l);
```

`bind(...)`:

- centers the part on the target bone
- points local `+Z` from `Start -> End`
- fits the part length to the bone for simple bound assets

## Robot Assets

Current built-in skeleton and robot part assets:

- `Robot`: simplified rigid biped with named joints like `head`, `hand_l`, `foot_r` and bones like `torso`, `forearm_l`, `thigh_r`
- `RobotSegment`: generic bindable limb segment along local `+Z`
- `RobotTorso`: torso block with `length`, `width`, and `depth`
- `RobotHead`: simple rounded head sphere
- `RobotJoint`: spherical robot joint part
- `RobotFoot`: forward-offset foot block
- `RobotBody`: assembled robot object that takes a `skeleton` plus materials and builds the full rigid robot body

See `examples/robot_on_stone.ft` in the repo for a posed robot standing on a procedural rock.

## RobotBody

You can either bind individual robot parts yourself, or let `RobotBody` do the assembly:

```forge
import "Robot";
import "RobotBody";

let rig = Robot {
  height: 1.9
}
  .attach(floor, Top);

let robot = RobotBody {
  skeleton: rig,
  material: Metal { color: #c8d0d8, roughness: 0.22 },
  accent_material: Material { color: #f4a261, roughness: 0.35 }
};
```
