ForgedThoughts is under early development.

ForgedThoughts is a modeling and rendering programming language utilizing SDFs.

# Example

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

sphere.material.rgb = F3("9F6F4A");
sphere.scale = 1.0;
let cone = Cone(0.3, 0.25, 0.0);

sphere.smin(cone, 0.5);

// Make it hollow

let cut_out = sphere.copy();
cut_out.position.y -= 0.04;
cut_out.scale = 0.98;

sphere -= cut_out;

// Eye holes

let eyes = Ellipsoid();
eyes.size = F3(0.10, 0.03, 0.1);
eyes.position.x = 0.07;
eyes.position.y -= 0.03;
eyes.position.z = 0.3;
eyes.mirror.x = true;
sphere -= eyes;

// Node and mouth

let cut = Box(F3(0.07, 0.2, 0.1));
cut.position.y -= 0.25;
cut.position.z = 0.2;

let modifier = RayModifier("x", "*", "sin", "y");
modifier.frequency = 10.0;
modifier.amplitude = 0.7;
modifier.add = 1.0;
cut.modifier = modifier;

sphere -= cut;
```

![image](main.png)