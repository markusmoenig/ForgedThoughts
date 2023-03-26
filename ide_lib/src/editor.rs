use code_editor::prelude::*;
use crate::prelude::*;

pub enum Widgets {
    WidgetCodeToolbar,
}

use Widgets::*;

pub struct Editor {
    code_editor                 : CodeEditor,

    context                     : Context,

    widgets                     : Vec<Box<dyn Widget>>,

    preview_rect                : Rect,
    code_editor_rect            : Rect,

    divider                     : isize,

    is_cmd                      : bool,

    touch_in_code_toolbar       : bool,
    drag_code_toolbar_off       : isize,
    drag_code_toolbar_start     : isize,

    touch_in_code_editor        : bool,
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
        code_editor.set_font_size(15.0);

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

            divider                 : 0,

            is_cmd                  : false,

            touch_in_code_toolbar   : false,
            touch_in_code_editor    : false,

            drag_code_toolbar_start : 0,
            drag_code_toolbar_off   : 0,
        }
    }

    pub fn draw(&mut self, pixels: &mut [u8], width: usize, height: usize) {

        self.check_file_path_receivers();

        // --

        let code_toolbar_height = 30;

        self.context.width = width;
        self.context.height = height;

        // Divider sanity check

        if self.divider > (height / 2) as isize - code_toolbar_height as isize {
            self.divider = (height / 2) as isize - code_toolbar_height as isize;
        }

        let preview_height;
        if (height / 2) as isize + self.divider > 0 {
            preview_height = ((height / 2) as isize + self.divider) as usize;
        } else {
            preview_height = 0;
        }

        let code_editor_height = height - preview_height - code_toolbar_height;

        self.preview_rect = Rect::new(0, 0, width, preview_height);
        self.code_editor_rect = Rect::new(0, preview_height + code_toolbar_height, width, code_editor_height);

        self.widgets[0].set_rect(Rect::new(0, preview_height, width, code_toolbar_height));

        // Clear preview area
        self.context.draw2d.draw_rect(pixels, &self.preview_rect.to_tuple(), width, &[0, 0, 0, 0]);

        if let Some(rc) = self.context.check_render_progress() {
            if rc.0 {
                self.widgets[WidgetCodeToolbar as usize].process_cmd(WidgetCmd::BuildFinished, &mut self.context)
            }
            self.widgets[WidgetCodeToolbar as usize].process_cmd(WidgetCmd::BuildStatus(rc.1), &mut self.context)
        }

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
        self.touch_in_code_toolbar = false;
        self.touch_in_code_editor = false;

        let mut consumed = false;

        for w in &mut self.widgets {
            if w.touch_down(x, y, &mut self.context) {
                consumed = true;
            }
        }

        if consumed == false && self.widgets[0].contains(x, y) {
            self.touch_in_code_toolbar = true;
            self.drag_code_toolbar_start = self.divider;
            self.drag_code_toolbar_off = (self.context.height / 2) as isize - y as isize;
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

        if self.touch_in_code_toolbar {
            self.divider = self.drag_code_toolbar_start -((self.context.height / 2) as isize - y as isize - self.drag_code_toolbar_off);
        }

        if self.touch_in_code_editor && self.code_editor_rect.is_inside_f32((x, y)) {
            if self.code_editor.mouse_dragged((x as usize - self.code_editor_rect.x, y as usize - self.code_editor_rect.y)) {
                return true
            }
        }

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
            self.context.cmd = Some(Command::Build);
        } else
        if self.code_editor.key_down(char, key) {
            return true;
        }

        self.process_cmds();

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
        if let Some(cmd) = &self.context.cmd {
            match cmd {
                Command::Build => {
                    let code = self.code_editor.get_text();
                    self.context.start_render(code);
                    self.widgets[0].process_cmd(WidgetCmd::BuildStarted, &mut self.context);
                }
            }
        }
        self.context.cmd = None;
    }

    /// Open a file requester
    pub fn open(&mut self) {
        self.context.open_file_dialog();
    }

    /// Save the file
    pub fn save(&mut self) {
        if let Some(path) = &self.context.file_path {
            std::fs::write(path, self.code_editor.get_text()).expect("Unable to write file");
        } else {
            self.save_as();
        }
    }

    /// Save the file as...
    pub fn save_as(&mut self) {
        self.context.save_file_dialog();
    }

    /// Save the image as...
    pub fn save_image_as(&mut self) {
        if self.context.u8_buffer.is_empty() == false {
            self.context.save_image_dialog();
        }
    }

    // Cut / Copy / Paste

    pub fn cut(&mut self) -> String {
        self.code_editor.cut()
    }

    pub fn copy(&mut self) -> String {
        self.code_editor.copy()
    }

    pub fn paste(&mut self, text:String) {
        self.code_editor.paste(text);
    }

    fn _get_time(&self) -> u128 {
        let stop = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards");
            stop.as_millis()
    }

    /// Check the file receivers
    fn check_file_path_receivers(&mut self) {

        // Have to read a file from file open dialog ?
        if let Some(rx) = &self.context.file_path_receiver {
            let rc = rx.try_recv();
            if let Some(path) = rc.ok() {
                if let Some(p) = path {
                    if let Some(code) = std::fs::read_to_string(p.clone()).ok() {
                        self.code_editor.set_text(code);
                        self.context.file_path = Some(p);
                    }
                }
                self.context.file_path_receiver = None;
            }
        } else
        // Have to save a file from file save dialog ?
        if let Some(rx) = &self.context.save_file_path_receiver {
            let rc = rx.try_recv();
            if let Some(path) = rc.ok() {
                if let Some(p) = path {
                    std::fs::write(p.clone(), self.code_editor.get_text()).expect("Unable to write file");
                    self.context.file_path = Some(p);
                }
                self.context.save_file_path_receiver = None;
            }
        } else
        // Have to save a png from imge save dialog ?
        if let Some(rx) = &self.context.save_image_path_receiver {
            let rc = rx.try_recv();
            if let Some(path) = rc.ok() {
                if let Some(p) = path {

                    if self.context.u8_buffer.is_empty() == false {
                        // Write it to file

                        use std::fs::File;
                        use std::io::BufWriter;

                        let file = File::create(p).unwrap();
                        let ref mut w = BufWriter::new(file);

                        let mut encoder = png::Encoder::new(w, self.context.u8_width as u32, self.context.u8_height as u32);
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        // Adding text chunks to the header
                        encoder
                            .add_text_chunk(
                                "ForgedThoughts".to_string(),
                                "This image was generated by ForgedThoughts.com".to_string(),
                            )
                            .unwrap();

                        let mut writer = encoder.write_header().unwrap();

                        writer.write_image_data(&self.context.u8_buffer).unwrap();
                    }
                }
                self.context.save_image_path_receiver = None;
            }
        }
    }
}