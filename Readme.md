ForgedThoughts is under early development.

ForgedThoughts is a modeling and rendering programming language utilizing SDFs.

# Example

```rust
fn background(uv) {
    return F3(uv.x, uv.y, 0.0);
}

camera.origin.x = 2.0;

settings.background = F3(1.0, 1.0, 1.0);
settings.antialias = 3;

let light = PointLight();
light.position = F3(3.0, 3.0, 5.0);
light.intensity = 1.0;

let sphere1 = Sphere();
sphere1.radius = 1.2;
sphere1.material.rgb = F3(0.2, 0.6, 0.8);

let sphere2 = Sphere();
sphere2.position.x = 1.0;
sphere2.radius = 1.2;

sphere1 -= sphere2;
```

![image](main.png)