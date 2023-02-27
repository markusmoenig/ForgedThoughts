
ForgedThoughts is a modeling and rendering programming language utilizing SDFs and is in early development.

For documentation and examples see the [Website](https://forgedthoughts.com).

![image](main.png)

## Goals

Forged Thoughts strives to create high-quality distance field models for rendering and poligonization. It utilizes multi-threaded CPU based rendering in 64-bit to prevent the limitations of SDFs on the GPU. The focus is on quality, rather than speed.

The overall project goals are:

* Create signed distance fields for rendering and poligonization.
* Focus is on quality rather than speed (although all example render in just a few hundred ms on my machine).
* CPU based rather than GPU based. All computation is done in 64-bit.
* Provide an easy but powerful syntax to model and render SDFs without any limitations.
* Access to all SDF modeling primitives, modifiers and tricks (In progress).
* Various integrated renderers (TODO)\
* Animation( TODO)
* Object hierarchies by including sub-class scripts (TODO)
* Share objects and materials via an integrated database (TODO)
* Model and work with 2D SDFs and Text as an overlay to the 3D layer (TODO)
* Terrain (TODO)
* Physics (TODO)

## Example

The above helmet was created with the following code:

```rust
camera.origin.z = 0.75;

settings.width = 600;
settings.height = 600;

settings.background = F3("444");
settings.antialias = 5;
//settings.opacity = 0.0;

let phong = Phong();
phong.specular = F3(0.5, 0.5, 0.5);
settings.renderer = phong;

let light = PointLight();
light.position = F3(3.0, 3.0, 5.0);
light.intensity = 1.5;

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
eyes.size = F3(0.10, 0.03, 0.1);
eyes.position.x = 0.07;
eyes.position.y -= 0.03;
eyes.position.z = 0.3;
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
