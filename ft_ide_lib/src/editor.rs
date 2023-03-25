use code_editor::prelude::*;
use forgedthoughts::prelude::*;

use crate::prelude::*;

pub struct Editor {
    code_editor         : CodeEditor,

    context             : Context,

    ft                  : FT,

    preview_rect        : Rect,
    code_editor_rect    : Rect,

    u8_buffer           : Vec<u8>,

    is_cmd              : bool,

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

        code_editor.set_text("".to_string());

        Self {

            code_editor,
            context,

            ft                      : FT::new(),

            preview_rect            : Rect::empty(),
            code_editor_rect        : Rect::empty(),

            u8_buffer               : vec![],

            is_cmd                  : false,

            offset                  : (0.0, 0.0),
            hover_pos               : None,

            scale                   : 3.0,

            td_pos                  : (0.0, 0.0),
            td_offset               : (0.0, 0.0),
        }
    }

    pub fn draw(&mut self, pixels: &mut [u8], width: usize, height: usize) {
        self.context.width = width;
        self.context.height = height;

        self.preview_rect = Rect::new(0, 0, width, height / 2);
        self.code_editor_rect = Rect::new(0, height / 2, width, height / 2);

        if self.u8_buffer.is_empty() == false {
            pixels.copy_from_slice(&self.u8_buffer);
        }

        self.code_editor.draw(pixels, self.code_editor_rect.to_tuple(), width);
    }

    pub fn touch_down(&mut self, x: f32, y: f32) -> bool {

        if self.code_editor_rect.is_inside_f32((x, y)) {
            if self.code_editor.mouse_down((x as usize - self.code_editor_rect.x, y as usize - self.code_editor_rect.y)) {
                return true
            }
        }
        false
    }

    pub fn touch_dragged(&mut self, x: f32, y: f32) -> bool {

        if self.code_editor_rect.is_inside_f32((x, y)) {
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

        if self.code_editor_rect.is_inside_f32((x, y)) {
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

            let rc = self.ft.compile_code(code, "main.ft".into());
            if rc.is_ok() {
                if let Some(mut ctx) = rc.ok() {
                    let mut buffer = ColorBuffer::new(self.context.width, self.context.height);
                    self.ft.render(&mut ctx, &mut buffer);

                    self.u8_buffer = buffer.to_u8_vec();
                    return true;
                }
            } else {
                println!("{:?}", rc.err());
            }
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