use crate::prelude::*;

pub mod code_toolbar;

#[allow(unused)]
pub trait Widget : Sync + Send {

    fn new() -> Self where Self: Sized;

    fn set_rect(&mut self, rect: Rect);
    fn set_string(&mut self, string: String) { }

    fn draw(&mut self, pixels: &mut [u8], context: &mut Context);

    fn contains(&mut self, x: f32, y: f32) -> bool {
        false
    }

    fn touch_down(&mut self, x: f32, y: f32, context: &mut Context) -> bool {
        false
    }

    fn touch_dragged(&mut self, x: f32, y: f32, context: &mut Context) -> bool {
        false
    }

    fn touch_up(&mut self, _x: f32, _y: f32, context: &mut Context) -> bool {
        false
    }
}
