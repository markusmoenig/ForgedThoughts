use crate::prelude::*;
use forgedthoughts::prelude::*;

use fontdue::Font;
use std::path::PathBuf;
use rustc_hash::FxHashMap;

use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;

#[derive(PartialEq, Clone, Debug)]
pub enum RenderProgress {
    Compiled(String),
    Frame(u32, Vec<u8>, usize, usize, String),
    Finished(String),
}

use RenderProgress::*;

#[derive(PartialEq, Clone, Debug)]
pub enum Command {
    Build,
}

pub struct Context {
    pub draw2d                      : Draw2D,

    pub width                       : usize,
    pub height                      : usize,

    pub color_widget                : [u8;4],
    pub color_state_selected        : [u8;4],
    pub color_state_normal          : [u8;4],
    pub color_text                  : [u8;4],
    pub color_text_disabled         : [u8;4],

    // Current Frame

    pub u8_buffer                   : Vec<u8>,
    pub u8_width                    : usize,
    pub u8_height                   : usize,
    pub u8_receiver                 : Option<Receiver<RenderProgress>>,

    // Fonts
    pub fonts                       : FxHashMap<String, Font>,

    pub code_editor_font            : Option<Font>,
    pub ui_font                     : Option<Font>,

    pub font_size                   : f32,

    // Icons
    pub icons                       : FxHashMap<String, (Vec<u8>, u32, u32)>,

    // Current file
    pub file_path                   : Option<PathBuf>,

    // File to open
    pub file_path_receiver          : Option<Receiver<Option<PathBuf>>>,

    // File to save
    pub save_file_path_receiver     : Option<Receiver<Option<PathBuf>>>,

    // Image to save
    pub save_image_path_receiver    : Option<Receiver<Option<PathBuf>>>,

    pub cmd                         : Option<Command>,
}

impl Context {

    pub fn new() -> Self {

        let mut code_editor_font : Option<Font> = None;
        let mut ui_font : Option<Font> = None;

        let mut icons : FxHashMap<String, (Vec<u8>, u32, u32)> = FxHashMap::default();
        let mut fonts : FxHashMap<String, Font> = FxHashMap::default();

        for file in Embedded::iter() {
            let name = file.as_ref();
            if name.ends_with(".ttf") {
                if let Some(font_bytes) = Embedded::get(name) {
                    if let Some(f) = Font::from_bytes(font_bytes.data, fontdue::FontSettings::default()).ok() {
                        let name = std::path::Path::new(&name).file_stem().unwrap().to_str().unwrap();
                        if name == "SourceCodePro-Regular" {
                            code_editor_font = Some(f.clone());
                        }
                        if name == "Roboto-Regular" {
                            ui_font = Some(f.clone());
                        }
                        fonts.insert(name.to_string(), f);
                    }
                }
            } else
            if name.starts_with("icons/") {
                if let Some(icon_bytes) = Embedded::get(name) {

                    let data = std::io::Cursor::new(icon_bytes.data);

                    let decoder = png::Decoder::new(data);
                    if let Ok(mut reader) = decoder.read_info() {
                        let mut buf = vec![0; reader.output_buffer_size()];
                        let info = reader.next_frame(&mut buf).unwrap();
                        let bytes = &buf[..info.buffer_size()];


                        let name = std::path::Path::new(&name).file_stem().unwrap().to_str().unwrap();
                        icons.insert(name.to_owned(), (bytes.to_vec(), info.width, info.height));
                    }
                }
            }
        }

        Self {
            draw2d                  : Draw2D::new(),

            width                   : 0,
            height                  : 0,

            color_widget            : [30, 30, 32, 255],
            color_state_selected    : [135, 135, 135, 255],
            color_state_normal      : [80, 80, 80, 255],
            color_text              : [244, 244, 244, 255],
            color_text_disabled     : [100, 100, 100, 255],

            u8_buffer               : vec![],
            u8_width                : 0,
            u8_height               : 0,
            u8_receiver             : None,

            fonts,
            code_editor_font,
            ui_font,

            font_size               : 16.0,

            icons,

            file_path               : None,
            file_path_receiver      : None,

            save_file_path_receiver : None,
            save_image_path_receiver: None,

            cmd                     : None
        }
    }

    // Start Render

    pub fn start_render(&mut self, code: String) {
        let (tx, rx): (Sender<RenderProgress>, Receiver<RenderProgress>) = mpsc::channel();

        let _handle = std::thread::spawn( move || {

            fn get_time() -> u128 {
                let stop = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards");
                    stop.as_millis()
            }

            let ft = FT::new();
            let rc = ft.compile_code(code, "main.ft".into());

            if rc.is_ok() {
                if let Some(mut ctx) = rc.ok() {

                    tx.send(Compiled("Compiled succcessfully.".into())).unwrap();

                    let mut total_time = 0;

                    let mut buffer = ColorBuffer::new(ctx.settings.width as usize, ctx.settings.width as usize);

                    let is_path_traced = ctx.settings.renderer.iterations > 1;

                    for i in 0..ctx.settings.renderer.iterations {

                        let frame_start = get_time();

                        ft.render(&mut ctx, &mut buffer);
                        total_time += get_time() - frame_start;

                        let b = buffer.to_u8_vec();

                        let status;

                        if is_path_traced == false {
                            status = format!("Rendered ({}x{}, {})", ctx.settings.width, ctx.settings.height, total_time);
                        } else {
                            let avg = (total_time as f64 / (i+1) as f64) / 1000.0;
                            status = format!("Rendered Frame {} ({}x{}, Average: {:.2}s)", i + 1, ctx.settings.width, ctx.settings.height, avg);
                        }

                        tx.send(Frame(i as u32, b, ctx.settings.width as usize, ctx.settings.height as usize, status)).unwrap();
                    }
                    tx.send(Finished("Ready.".into())).unwrap();
                }
            } else {
                let status;
                if let Some(error) = rc.err() {
                    status = error;
                } else {
                    status = "Unknown Error".to_string();
                }
                tx.send(Finished(status)).unwrap();
            }
        });

        self.u8_receiver = Some(rx);
    }

    /// Check if a renderer is finished
    pub fn check_render_progress(&mut self) -> Option<(bool, String)> {

        if let Some(receiver) = &self.u8_receiver {
            let rc = receiver.try_recv();
            if let Some(result) = rc.ok() {

                match result {
                    Compiled(status) => {
                        return Some((false, status));
                    },
                    Frame(_frame_nr, buffer, width, height, status) => {
                        self.u8_buffer = buffer;
                        self.u8_width = width;
                        self.u8_height = height;
                        return Some((false, status));
                    },
                    Finished(status) => {
                        self.u8_receiver = None;
                        return Some((true, status));
                    }
                }
            }
        }
        None
    }

    /// Open file dialog
    pub fn open_file_dialog(&mut self) {
        let (tx, rx): (Sender<Option<PathBuf>>, Receiver<Option<PathBuf>>) = mpsc::channel();

        let task = rfd::AsyncFileDialog::new()
            .add_filter("FT", &["ft"])
            .set_title("Choose FT File")
            .pick_file();

        std::thread::spawn(move || {
            let file = futures::executor::block_on(task);
            if let Some(file) = file {
                tx.send(Some(file.path().to_path_buf())).unwrap();
            } else {
                tx.send(None).unwrap();
            }
        });

        self.file_path_receiver = Some(rx);
    }

    /// Save file dialog
    pub fn save_file_dialog(&mut self) {
        let (tx, rx): (Sender<Option<PathBuf>>, Receiver<Option<PathBuf>>) = mpsc::channel();

        let task = rfd::AsyncFileDialog::new()
            .add_filter("FT", &["ft"])
            .set_title("Choose FT File")
            .save_file();

        std::thread::spawn(move || {
            let file = futures::executor::block_on(task);
            if let Some(file) = file {
                tx.send(Some(file.path().to_path_buf())).unwrap();
            } else {
                tx.send(None).unwrap();
            }
        });

        self.save_file_path_receiver = Some(rx);
    }

    /// Save image dialog
    pub fn save_image_dialog(&mut self) {
        let (tx, rx): (Sender<Option<PathBuf>>, Receiver<Option<PathBuf>>) = mpsc::channel();

        let task = rfd::AsyncFileDialog::new()
            .add_filter("PNG", &["png"])
            .set_title("Choose PNG File")
            .save_file();

        std::thread::spawn(move || {
            let file = futures::executor::block_on(task);
            if let Some(file) = file {
                tx.send(Some(file.path().to_path_buf())).unwrap();
            } else {
                tx.send(None).unwrap();
            }
        });

        self.save_image_path_receiver = Some(rx);
    }
}