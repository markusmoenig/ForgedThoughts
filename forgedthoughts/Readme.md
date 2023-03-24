For documentation and examples please visit the [Website](https://forgedthoughts.com).

You can render a Forged Thoughts script in your application like this:

```rust
use forgedthoughts::prelude::*;

// Create FT
let ft = FT::new();

// Compile a script
let rc = ft.compile_code("let s = Sphere();".to_string(), "main.ft".to_string);

// Render it into a buffer
let mut buffer = ColorBuffer::new(ctx.settings.width as usize, ctx.settings.height as usize);

for i in 0..ctx.settings.renderer.iterations {
    ft.render(&mut ctx, &mut buffer);
}

// Convert the buffer to Vec<u8>
let out = buffer.to_u8_vec();
```