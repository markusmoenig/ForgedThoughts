#[derive(Clone)]
pub struct ColorBuffer<T> {
    pub pixels          : Vec<T>,
    pub size            : [usize; 2],
}

impl<T: Clone> ColorBuffer<T> {
    pub fn new (width: usize, height: usize, fill: T) -> Self {

        Self {
            pixels      : vec![fill; width * height * 4],
            size        : [width, height]
        }
    }
}