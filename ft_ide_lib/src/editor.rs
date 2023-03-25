use code_editor::prelude::*;

use crate::prelude::*;
pub struct Editor {
    code_editor         : CodeEditor,

    context             : Context,

    widgets             : Vec<Box<dyn Widget>>,

    preview_rect        : Rect,
    code_editor_rect    : Rect,

    is_cmd              : bool,

    touch_in_code_editor: bool,

    offset              : (f32, f32),
    hover_pos           : Option<(i32, i32)>,

    td_pos              : (f32, f32),
    td_offset           : (f32, f32),

    scale               : f32,
}

unsafe impl Send for Editor {}
unsafe impl Sync for Editor {}

impl Editor {

    pub fn new() -> Self {

        let mut code_editor = CodeEditor::new();
        let context = Context::new();

        if let Some(f) = &context.code_editor_font {
            code_editor.set_font_data(f.clone());
        }

        code_editor.set_text("settings.width = 600;
settings.height = 600;

let l = PointLight();

let s = Sphere();".to_string());

        let mut widgets : Vec<Box<dyn Widget>> = vec![];

        let mut code_toolbar = Box::new(CodeToolbar::new());
        code_toolbar.rect.height = 30;

        widgets.push(code_toolbar);

        Self {
            code_editor,
            context,

            widgets,

            preview_rect            : Rect::empty(),
            code_editor_rect        : Rect::empty(),

            is_cmd                  : false,

            touch_in_code_editor    : false,

            offset                  : (0.0, 0.0),
            hover_pos               : None,

            scale                   : 3.0,

            td_pos                  : (0.0, 0.0),
            td_offset               : (0.0, 0.0),
        }
    }

    pub fn draw(&mut self, pixels: &mut [u8], width: usize, height: usize) {

        let CODE_TOOLBAR_HEIGHT = 30;

        self.context.width = width;
        self.context.height = height;

        let preview_height = height / 2;
        let code_editor_height = height / 2 - CODE_TOOLBAR_HEIGHT;

        self.preview_rect = Rect::new(0, 0, width, preview_height);
        self.code_editor_rect = Rect::new(0, preview_height + CODE_TOOLBAR_HEIGHT, width, code_editor_height);

        self.widgets[0].set_rect(Rect::new(0, preview_height, width, CODE_TOOLBAR_HEIGHT));

        // Clear preview area
        self.context.draw2d.draw_rect(pixels, &self.preview_rect.to_tuple(), width, &[0, 0, 0, 0]);

        self.context.check_render_progress();

        // Display preview if any
        if self.context.u8_buffer.is_empty() == false {
            if self.context.u8_width < self.preview_rect.width && self.context.u8_height < self.preview_rect.height {
                let x_off = (self.preview_rect.width - self.context.u8_width) / 2;
                let y_off = (self.preview_rect.height - self.context.u8_height) / 2;

                self.context.draw2d.copy_slice(pixels, &self.context.u8_buffer, &(x_off, y_off, self.context.u8_width, self.context.u8_height), width)
            } else {
                let ratio = (self.preview_rect.width as f32 / self.context.u8_width as f32).min(self.preview_rect.height as f32 / self.context.u8_height as f32);

                let width = (self.context.u8_width as f32 * ratio) as usize;
                let height = (self.context.u8_height as f32 * ratio) as usize;

                let x_off = (self.preview_rect.width - width) / 2;
                let y_off = (self.preview_rect.height - height) / 2;

                self.context.draw2d.scale_chunk(pixels, &(x_off, y_off, width, height), self.context.width, &self.context.u8_buffer, &(self.context.u8_width, self.context.u8_height), 1.0);
            }
        }

        // Draw the widgets
        for w in &mut self.widgets {
            w.draw(pixels, &mut self.context);
        }

        // and the code editor
        self.code_editor.draw(pixels, self.code_editor_rect.to_tuple(), width);
    }

    pub fn touch_down(&mut self, x: f32, y: f32) -> bool {
        self.touch_in_code_editor = false;

        let mut consumed = false;

        for w in &mut self.widgets {
            if w.touch_down(x, y, &mut self.context) {
                consumed = true;
            }
        }

        if consumed == false && self.code_editor_rect.is_inside_f32((x, y)) {
            let x = x as usize - self.code_editor_rect.x;
            let y = y as usize - self.code_editor_rect.y;
            if self.code_editor.mouse_down((x, y)) {
                self.touch_in_code_editor = true;
                return true
            }
        }

        self.process_cmds();
        consumed
    }

    pub fn touch_dragged(&mut self, x: f32, y: f32) -> bool {

        if self.touch_in_code_editor && self.code_editor_rect.is_inside_f32((x, y)) {
            if self.code_editor.mouse_dragged((x as usize - self.code_editor_rect.x, y as usize - self.code_editor_rect.y)) {
                return true
            }
        }

        self.offset.0 = self.td_offset.0 - (self.td_pos.0 - x);
        self.offset.1 = self.td_offset.1 - (self.td_pos.1 - y);

        self.process_cmds();

        true
    }

    pub fn touch_up(&mut self, x: f32, y: f32) -> bool {

        if self.touch_in_code_editor && self.code_editor_rect.is_inside_f32((x, y)) {
            if self.code_editor.mouse_up((x as usize - self.code_editor_rect.x, y as usize - self.code_editor_rect.y)) {
                return true
            }
        }

        self.process_cmds();
        false
    }

    pub fn hover(&mut self, _x: f32, _y: f32) -> bool {

        false
    }

    pub fn key_down(&mut self, char: Option<char>, key: Option<WidgetKey>) -> bool {

        if self.is_cmd && char == Some('b') {
            let code = self.code_editor.get_text();
            self.context.start_render(code);
        } else
        if self.code_editor.key_down(char, key) {
            return true;
        }

        false
    }

    pub fn mouse_wheel(&mut self, delta: (isize, isize)) -> bool {
        if self.code_editor.mouse_wheel(delta) {
            return true;
        }
        false
    }

    pub fn modifier_changed(&mut self, shift: bool, ctrl: bool, alt: bool, logo: bool) -> bool {
        self.is_cmd = ctrl || logo;
        self.code_editor.modifier_changed(shift, ctrl, alt, logo);
        false
    }

    pub fn dropped_file(&mut self, _path: String) -> bool {
        false
    }

    /// Process possible UI commands
    fn process_cmds(&mut self) {

    }

    fn _get_time(&self) -> u128 {
        let stop = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
            stop.as_millis()
    }
}