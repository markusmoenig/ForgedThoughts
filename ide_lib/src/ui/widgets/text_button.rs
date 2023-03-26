use crate::prelude::*;

pub struct TextButton {

    pub rect            : Rect,
    buffer              : Vec<u8>,
    pub dirty           : bool,

    pub clicked         : bool,
    pub text            : String,
    pub rounding        : f64,
}

impl Widget for TextButton {

    fn new() -> Self {

        Self {

            rect        : Rect::empty(),
            buffer      : vec![],
            dirty       : true,

            clicked     : false,
            text        : "Button".to_string(),
            rounding    : 0.0,
        }
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        if self.buffer.len() != self.rect.width * self.rect.height * 4 {
            self.buffer = vec![0;self.rect.width * self.rect.height * 4];
        }
    }

    fn draw(&mut self, pixels: &mut [u8], context: &mut Context) {

        let r = self.rect.to_sized_tuple();

        if self.dirty {
            let color = if self.clicked  { context.color_state_selected } else { context.color_state_normal };
            let border_color = if !self.clicked  { context.color_state_selected } else { context.color_state_normal };

            context.draw2d.draw_rect(&mut self.buffer, &r, self.rect.width, &[0, 0, 0, 0]);
            context.draw2d.draw_rounded_rect_with_border(&mut self.buffer, &r, self.rect.width, &color, &(self.rounding, self.rounding, self.rounding, self.rounding),&border_color, 1.5);

            context.draw2d.blend_text_rect(&mut self.buffer, &r, self.rect.width, &context.ui_font.as_ref().unwrap(), context.font_size, &self.text, &context.color_text, TextAlignment::Center);
        }

        self.dirty = false;
        context.draw2d.blend_slice(pixels, &self.buffer, &self.rect.to_tuple(), context.width);
    }

    fn contains(&mut self, x: f32, y: f32) -> bool {
        if self.rect.is_inside((x as usize, y as usize)) {
            true
        } else {
            false
        }
    }

    fn touch_down(&mut self, x: f32, y: f32, context: &mut Context) -> bool {

        if self.rect.is_inside((x as usize, y as usize)) {
            self.dirty = true;
            self.clicked = true;
            context.cmd = Some(Command::Build);
            return true;
        }

        false
    }

    /*
    fn touch_dragged(&mut self, x: f32, y: f32, context: &mut Context) -> bool {


        true
    }

    fn touch_up(&mut self, _x: f32, _y: f32, context: &mut Context) -> bool {
        false
    }*/
}