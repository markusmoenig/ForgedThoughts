use crate::prelude::*;
use forgedthoughts::prelude::*;

use fontdue::Font;
//use std::path::PathBuf;
use rustc_hash::FxHashMap;

use std::sync::mpsc::{Sender, Receiver};
use std::sync::mpsc;

#[derive(PartialEq, Clone, Debug)]
pub enum RenderProgress {
    Frame(u32, Vec<u8>, usize, usize),
    Finished(),
}

use RenderProgress::*;

#[derive(PartialEq, Clone, Debug)]
pub enum Command {
    PerspectiveSwitch,
    ButtonDown(String),
}

pub struct Context {
    pub draw2d              : Draw2D,

    pub width               : usize,
    pub height              : usize,

    pub color_widget        : [u8;4],
    pub color_selected      : [u8;4],
    pub color_text          : [u8;4],
    pub color_text_disabled : [u8;4],

    // Current Frame

    pub u8_buffer           : Vec<u8>,
    pub u8_width            : usize,
    pub u8_height           : usize,
    pub u8_receiver         : Option<Receiver<RenderProgress>>,

    // Fonts
    pub fonts               : FxHashMap<String, Font>,

    pub code_editor_font    : Option<Font>,
    pub ui_font             : Option<Font>,

    // Icons
    pub icons               : FxHashMap<String, (Vec<u8>, u32, u32)>,

    pub cmd                 : Option<Command>,
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
                        if name == "OpenSans-Regular" {
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
            draw2d          : Draw2D::new(),

            width           : 0,
            height          : 0,

            color_widget    : [30, 30, 32, 255],
            color_selected  : [135, 135, 135, 255],
            color_text      : [244, 244, 244, 255],
            color_text_disabled : [100, 100, 100, 255],

            u8_buffer               : vec![],
            u8_width                : 0,
            u8_height               : 0,
            u8_receiver             : None,

            fonts,
            code_editor_font,
            ui_font,

            icons,

            cmd             : None
        }
    }

    // Start Render

    pub fn start_render(&mut self, code: String) {
        let (tx, rx): (Sender<RenderProgress>, Receiver<RenderProgress>) = mpsc::channel();

        let _handle = std::thread::spawn( move || {

            let ft = FT::new();
            let rc = ft.compile_code(code, "main.ft".into());

            if rc.is_ok() {
                if let Some(mut ctx) = rc.ok() {

                    let mut buffer = ColorBuffer::new(ctx.settings.width as usize, ctx.settings.width as usize);

                    for i in 0..ctx.settings.renderer.iterations {
                        ft.render(&mut ctx, &mut buffer);
                        let b = buffer.to_u8_vec();
                        tx.send(Frame(i as u32, b, ctx.settings.width as usize, ctx.settings.height as usize)).unwrap();
                    }
                    tx.send(Finished()).unwrap();
                }
            } else {
                println!("{:?}", rc.err());
            }
        });

        self.u8_receiver = Some(rx);
    }

    // Check if a renderer is finished
    pub fn check_render_progress(&mut self) {
        if let Some(receiver) = &self.u8_receiver {
            let rc = receiver.try_recv();
            if let Some(result) = rc.ok() {

                match result {
                    Frame(_frame_nr, buffer, width, height) => {
                        self.u8_buffer = buffer;
                        self.u8_width = width;
                        self.u8_height = height;
                    },
                    Finished() => {
                        self.u8_receiver = None;
                    }
                }
            }
        }
    }
}