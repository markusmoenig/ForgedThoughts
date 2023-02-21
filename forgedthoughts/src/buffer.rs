use crate::prelude::*;

#[derive(Clone)]
pub struct ColorBuffer {
    pub pixels          : Vec<F>,
    pub size            : [usize; 2],
}

impl ColorBuffer {
    pub fn new (width: usize, height: usize, fill: F) -> Self {

        Self {
            pixels      : vec![fill; width * height * 4],
            size        : [width, height]
        }
    }

    /// Convert the frame to an u8 vec
    pub fn to_u8_vec(&self) -> Vec<u8> {

        let [width, height] = self.size;

        let source = &self.pixels[..];
        let mut out : Vec<u8> = vec![0; self.size[0] * self.size[1] * 4];

        for y in 0..height {
            for x in 0..width {
                let d = x * 4 + y * width * 4;
                let c = [(source[d] * 255.0) as u8, (source[d+1] * 255.0) as u8,  (source[d+2] * 255.0) as u8,  (source[d+3] * 255.0) as u8];
                out[d..d + 4].copy_from_slice(&c);
            }
        }

        out
    }
}