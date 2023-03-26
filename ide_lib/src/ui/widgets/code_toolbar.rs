use crate::prelude::*;

pub struct CodeToolbar {
    pub rect            : Rect,
    buffer              : Vec<u8>,
    pub dirty           : bool,

    status              : String,

    pub control_button  : TextButton,
}

impl Widget for CodeToolbar {

    fn new() -> Self {

        let mut control_button = TextButton::new();
        control_button.text = "Build".to_string();
        control_button.rounding = 6.0;

        Self {
            rect                : Rect::empty(),
            buffer              : vec![],
            dirty               : true,

            status              : "".into(),

            control_button,
        }
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;

        let control_rect = Rect::new( 10, 5, 80, rect.height - 8);
        self.control_button.set_rect(control_rect);

        if self.buffer.len() != self.rect.width * self.rect.height * 4 {
            self.buffer = vec![0;self.rect.width * self.rect.height * 4];
        }

        self.dirty = true;
    }

    fn draw(&mut self, pixels: &mut [u8], context: &mut Context) {

        let r = self.rect.to_sized_tuple();

        if self.dirty {
            context.draw2d.draw_rect(&mut self.buffer, &r, context.width, &context.color_widget);
            context.draw2d.draw_rect(&mut self.buffer, &(0, 0, r.2, 1), context.width, &context.color_state_normal);
            self.control_button.draw(&mut self.buffer, context);

            if self.status.is_empty() == false {
                let mut sr = r.clone();
                sr.0 = 100;
                sr.2 -= 110;
                context.draw2d.blend_text_rect(&mut self.buffer, &sr, self.rect.width, &context.ui_font.as_ref().unwrap(), 15.0, &self.status, &context.color_state_selected, TextAlignment::Right);
            }
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

            if self.control_button.touch_down(x - self.rect.x as f32, y - self.rect.y as f32, context) {
                self.dirty = true;
                return true;
            }
        }

        false
    }

    fn process_cmd(&mut self, cmd: WidgetCmd, _context: &mut Context) {
        match cmd {
            WidgetCmd::BuildStarted => {
                self.dirty = true;
                self.control_button.dirty = true;
                self.control_button.clicked = true;
                self.control_button.text = "Stop".into();
            },
            WidgetCmd::BuildFinished => {
                self.dirty = true;
                self.control_button.dirty = true;
                self.control_button.clicked = false;
                self.control_button.text = "Build".into();
            },
            WidgetCmd::BuildStatus(status) => {
                self.dirty = true;
                self.status = status;
            }
        }
    }

}