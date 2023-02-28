
Forged Thoughts is a modeling and rendering programming language. It is open source under the MIT license and currently in early development. The language utilizes 3D and 2D SDFs and is written in Rust and can be easily installed as a Rust subcommand.

For documentation and examples see the [Website](https://forgedthoughts.com).

![image](main.png)


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

The above helmet was created with the following code:

```rust
// Main shape

let sphere = Sphere(0.24);
let cone = Cone(0.3, 0.25, 0.0);

let helmet = smin(sphere, cone, 0.5);
helmet.material.rgb = F3("9F6F4A");

// Make it hollow

let cut_out = helmet.copy();
cut_out.position.y -= 0.04;
cut_out.scale = 0.98;

helmet -= cut_out;

// Eye holes

let eyes = Ellipsoid();
eyes.size = F3(0.11, 0.03, 0.1);
eyes.position = F3(0.06, -0.03, 0.3);
eyes.mirror.x = true;
helmet -= eyes;

// Nose and mouth

let cut = Box(F3(0.07, 0.2, 0.1));
cut.position.y -= 0.25;
cut.position.z = 0.2;

let modifier = RayModifier("x", "*", "sin", "y");
modifier.frequency = 10.0;
modifier.amplitude = 0.7;
modifier.addend = 1.0;
cut.modifier = modifier;
helmet -= cut;

// Stripe

let stripe = Box(F3(0.011, 0.17, 0.2));
stripe.position.y = 0.16;
stripe.position.z = 0.2;
helmet += Groove(stripe, 0.01, 0.02);
```

## Supporting Forged Thoughts

You can support the Forged Thoughts project by becoming a [GitHub Sponsor](https://github.com/sponsors/markusmoenig).

## License

Forged Thoughts is licensed under the MIT.

Unless explicitly stated otherwise, any contribution intentionally submitted for inclusion in Forged Thoughts, shall be MIT licensed as above, without any additional terms or conditions.

## Sponsors

None yet
