use crate::prelude::*;

pub struct CodeToolbar {

    pub rect            : Rect
}

impl Widget for CodeToolbar {

    fn new() -> Self {

        Self {
            rect        : Rect::empty()
        }
    }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn draw(&mut self, pixels: &mut [u8], context: &mut Context) {

        let mut r = self.rect.to_tuple();

        context.draw2d.draw_rect(pixels, &r, context.width, &context.color_widget);

        /*
        let mut color = if context.curr_perspective == Perspective::Iso { context.color_selected } else { context.color_widget };

        context.draw2d.draw_rounded_rect(pixels, &r, context.width, &color, &(10.0, 10.0, 10.0, 10.0));

        if let Some(font) = &context.ui_font {
            let mut rt = r.clone();
            rt.2 /= 2;
            rt.0 += rt.2;
            context.draw2d.blend_text_rect(pixels, &rt, context.width, &font, 18.0, "ISO", &context.color_text, crate::ui::draw2d::TextAlignment::Center)
        }

        color = if context.curr_perspective == Perspective::Iso { context.color_widget } else { context.color_selected };

        r.2 /= 2;
        context.draw2d.draw_rounded_rect(pixels, &r, context.width, &color, &(0.0, 0.0, 10.0, 10.0));

        if let Some(font) = &context.ui_font {
            context.draw2d.blend_text_rect(pixels, &r, context.width, &font, 18.0, "TOP", &context.color_text, crate::ui::draw2d::TextAlignment::Center)
        }*/
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
            /*
            context.cmd = Some(Command::PerspectiveSwitch);
            if (x as u32) < self.rect.x + self.rect.width / 2  {
                context.curr_perspective = Perspective::Top;
                return true;
            } else {
                context.curr_perspective = Perspective::Iso;
                return true;
            }*/
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