use crate::prelude::*;

/// A color buffer holding an array of float pixels.
#[derive(PartialEq, Debug, Clone)]
pub struct RenderBuffer {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<F>,
    pub frames: usize,

    pub file_path: Option<std::path::PathBuf>,
}

impl RenderBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0.0; width * height * 4],
            frames: 0,
            file_path: None,
        }
    }

    /// Get the color of a pixel
    #[inline(always)]
    pub fn at(&self, x: usize, y: usize) -> Color {
        let i = y * self.width * 4 + x * 4;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    /// Set the color of a pixel
    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        let i = y * self.width * 4 + x * 4;
        self.pixels[i..i + 4].copy_from_slice(&color);
    }

    /// Copy the pixels from another buffer to this buffer
    pub fn copy_from(&mut self, x: usize, y: usize, other: &RenderBuffer) {
        for local_y in 0..other.height {
            for local_x in 0..other.width {
                let global_x = x + local_x;
                let global_y = y + local_y;

                if global_x >= self.width || global_y >= self.height {
                    continue;
                }

                let index = (global_y * self.width + global_x) * 4;
                let local_index = (local_y * other.width + local_x) * 4;
                self.pixels[index..index + 4]
                    .copy_from_slice(&other.pixels[local_index..local_index + 4]);
            }
        }
    }

    /// Copy and accumulate pixels from another buffer to this buffer
    pub fn accum_from(&mut self, x: usize, y: usize, other: &RenderBuffer, iteration: u32) {
        for local_y in 0..other.height {
            for local_x in 0..other.width {
                let global_x = x + local_x;
                let global_y = y + local_y;

                if global_x >= self.width || global_y >= self.height {
                    continue;
                }

                let index = (global_y * self.width + global_x) * 4;
                let local_index = (local_y * other.width + local_x) * 4;

                for i in 0..4 {
                    let old = self.pixels[index + i];
                    let new = other.pixels[local_index + i];
                    let factor = 1.0 / iteration as f32;
                    self.pixels[index + i] = old * (1.0 - factor) + new * factor;
                }
            }
        }
    }

    /// Convert the frame to an u8 vec, applying gamma correction
    pub fn to_u8_vec_gamma(&self) -> Vec<u8> {
        let source = &self.pixels[..];
        let mut out: Vec<u8> = vec![0; self.width * self.height * 4];
        let gamma_correction = 0.4545;

        for y in 0..self.height {
            for x in 0..self.width {
                let d = x * 4 + y * self.width * 4;
                let c = [
                    (source[d].powf(gamma_correction) * 255.0) as u8,
                    (source[d + 1].powf(gamma_correction) * 255.0) as u8,
                    (source[d + 2].powf(gamma_correction) * 255.0) as u8,
                    (source[d + 3] * 255.0) as u8,
                ];
                out[d..d + 4].copy_from_slice(&c);
            }
        }

        out
    }

    /// Convert the frame to an u8 vecc.
    pub fn to_u8_vec(&self) -> Vec<u8> {
        let source = &self.pixels[..];
        let mut out: Vec<u8> = vec![0; self.width * self.height * 4];

        for y in 0..self.height {
            for x in 0..self.width {
                let d = x * 4 + y * self.width * 4;
                let c = [
                    (source[d] * 255.0) as u8,
                    (source[d + 1] * 255.0) as u8,
                    (source[d + 2] * 255.0) as u8,
                    (source[d + 3] * 255.0) as u8,
                ];
                out[d..d + 4].copy_from_slice(&c);
            }
        }

        out
    }

    /// Save the buffer to a file as PNG
    pub fn save(&self, path: std::path::PathBuf) {
        let mut image = image::ImageBuffer::new(self.width as u32, self.height as u32);

        for y in 0..self.height {
            for x in 0..self.width {
                let i = y * self.width * 4 + x * 4;
                let c = image::Rgb([
                    (self.pixels[i] * 255.0) as u8,
                    (self.pixels[i + 1] * 255.0) as u8,
                    (self.pixels[i + 2] * 255.0) as u8,
                ]);
                image.put_pixel(x as u32, y as u32, c);
            }
        }

        image.save(path).unwrap();
    }
}
