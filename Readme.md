
# Rewrite in Progress

A complete rewrite of the old ForgedThoughts is currently in process with a radical new design.

The website and current readme's are out of date and will be updated soon.

---

Forged Thoughts is a modeling and rendering programming language. It is open source under the MIT license and currently in early development. The language utilizes 3D and 2D SDFs and is written in Rust and can be easily installed as a Rust subcommand.

For documentation and examples see the [Website](https://forgedthoughts.com).

Forged Thoughts strives to create high-quality distance field models for rendering and poligonization. It utilizes multi-threaded CPU based rendering in 64-bit to prevent the limitations of SDFs on the GPU. The focus is on quality, rather than speed.

## Features

* Easy to use programming language with special modeling functionality.
* Inbuild renderer for Phong, PBR and a full featured BSDF pathtracer.
* Polygonization of models (OBJ).
* 64-bit heavily multi-threaded ray-marcher running on the CPU.
* Access to all SDF modeling primitives, modifiers and tricks (In progress).

## Goals

The overall project goals are:

* Create signed distance fields for rendering and poligonization.
* Focus is on quality rather than speed (although all example render in just a few hundred ms on my machine).
* CPU based rather than GPU based. All computation is done in 64-bit.
* Provide an easy but powerful syntax to model and render SDFs without any limitations.
* Animation( TODO)
* Object hierarchies by including sub-class scripts (TODO)
* Share objects and materials via an integrated database (TODO)
* Model and work with 2D SDFs and Text as an overlay to the 3D layer (TODO)
* Terrain (TODO)
* Physics (TODO)

## Example

#### Wine Glass

![Wine Glass](examples/wine_glass.png)

The modeling of the glass, except the materials, is just 9 lines.

```rust
// Glass

let glass = Cone(0.6, 0.7, 0.6);
glass.rounding = 0.2;

glass.material.rgb = F3(1.0, 1.0, 1.0);
glass.material.roughness = 0.0;
glass.material.transmission = 1.0;
glass.material.ior = 1.50;

let interior = glass.copy();
interior.scale = 0.96;

// Fluid

let fluid = interior.copy();
fluid.material.rgb = F3("722F37").to_linear();
fluid.material.transmission = 1.0;
fluid.material.roughness = 0.5;
fluid.material.ior = 1.3443705; // Red Wine
fluid.material.clearcoat_gloss = 1.0;
fluid.material.sheen = 1.0;
fluid.material.sheen_tint = 1.0;
fluid.max.y = 0.0;

glass -= interior;

// Top: Smooth Cut Off & Gold Rim

let box = Box();
box.material.rgb = F3("d4af37");
box.material.metallic = 1.0;
box.material.roughness = 0.2;
box.position.y = 1.5;

// Smoothly subtract the box from the glass
glass -= Smooth(box, 0.01);

// Create a groove with the gold material of the box
glass += Groove(box, 0.001, 0.07);
```

#### Helmet

![Helmet](examples/helmet.png)

The above helmet was created with the following code:

```rust
// Main shape - We make a smooth blend between a sphere and a cone

let sphere = Sphere(0.24);
let cone = Cone(0.3, 0.25, 0.0);

let helmet = smin(sphere, cone, 0.5);

// Assign the material

helmet.material.rgb = F3("9F6F4A");
helmet.material.metallic = 0.7;
helmet.material.roughness = 0.3;

// Make it hollow by creating a copy, subtract it and move it down a bit
// to open the bottom

let cut_out = helmet.copy();
cut_out.position.y -= 0.04;
cut_out.scale = 0.98;
helmet -= cut_out;

// Eye holes - We mirror an Ellipsoid on the x-axis and subtract it.

let eyes = Ellipsoid();
eyes.size = F3(0.11, 0.03, 0.1);
eyes.position = F3(0.06, -0.03, 0.3);
eyes.mirror.x = true;
helmet -= eyes;

// Nose and mouth - We modify a box and subtract it.

let cut = Box(F3(0.07, 0.2, 0.1));
cut.position.y -= 0.25;
cut.position.z = 0.2;

let modifier = RayModifier("x", "*", "sin", "y");
modifier.frequency = 10.0;
modifier.amplitude = 0.7;
modifier.addend = 1.0;
cut.modifier = modifier;
helmet -= cut;

// Stripe - We add a positive groove in the intersection between
// the helmet and a box.

let stripe = Box(F3(0.011, 0.17, 0.2));
stripe.position.y = 0.16;
stripe.position.z = 0.2;
helmet += Groove(stripe, 0.01, 0.02);
```

## Current 3D SDF Primitives

<table>
  <tr>
    <td> <img src="examples/primitives/sphere.png"  alt="Sphere" width = 400px height = 300px ></td>
    <td> <img src="examples/primitives/box.png"  alt="Box" width = 400px height = 300px ></td>
   </tr>
   <tr>
    <td> <img src="examples/primitives/cone.png"  alt="Cone" width = 400px height = 300px ></td>
    <td> <img src="examples/primitives/ellipsoid.png"  alt="Ellipsoid" width = 400px height = 300px ></td>
  </tr>
   <tr>
    <td> <img src="examples/primitives/torus.png"  alt="Torus" width = 400px height = 300px ></td>
    <td> <img src="examples/primitives/cylinder.png"  alt="Cylinder" width = 400px height = 300px ></td>
  </tr>
</table>

## Current Booleans

<table>
  <tr>
    <td> <img src="examples/booleans/addition.png"  alt="Sphere" width = 400px height = 300px ></td>
    <td> <img src="examples/booleans/addition_smooth.png"  alt="Box" width = 400px height = 300px ></td>
   </tr>
   <tr>
    <td> <img src="examples/booleans/addition_groove.png"  alt="Box" width = 400px height = 300px ></td>
    <td> <img src="examples/booleans/subtraction.png"  alt="Ellipsoid" width = 400px height = 300px ></td>
  </tr>
    <td> <img src="examples/booleans/subtraction_smooth.png"  alt="Cone" width = 400px height = 300px ></td>
    <td> <img src="examples/booleans/subtraction_groove.png"  alt="Box" width = 400px height = 300px ></td>
   </tr>
  </tr>
    <td> <img src="examples/booleans/intersection.png"  alt="Cone" width = 400px height = 300px ></td>
    <td> <img src="examples/booleans/intersection_smooth.png"  alt="Box" width = 400px height = 300px ></td>
   </tr>
</table>

## Current Merging Functions

<table>
  <tr>
    <td> <img src="examples/merging/smin.png"  alt="Smin" width = 400px height = 300px ></td>
   </tr>
</table>

## Current Modifier

<table>
  <tr>
    <td> <img src="examples/modifier/twist.png"  alt="Sphere" width = 400px height = 300px ></td>
    <td> <img src="examples/modifier/mirror.png"  alt="Box" width = 400px height = 300px ></td>
   </tr>
    <td> <img src="examples/modifier/max.png"  alt="Max" width = 400px height = 300px ></td>
    <td> <img src="examples/modifier/onion.png"  alt="Onion" width = 400px height = 300px ></td>
   </tr>
</table>

## Supporting Forged Thoughts

You can support the Forged Thoughts project by becoming a [GitHub Sponsor](https://github.com/sponsors/markusmoenig).

## License

Forged Thoughts is licensed under the MIT.

Unless explicitly stated otherwise, any contribution intentionally submitted for inclusion in Forged Thoughts, shall be MIT licensed as above, without any additional terms or conditions.

## Sponsors

None yet
