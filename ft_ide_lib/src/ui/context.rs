use crate::prelude::*;
use fontdue::Font;
use std::path::PathBuf;
use rustc_hash::FxHashMap;

#[derive(PartialEq, Clone, Debug)]
pub enum Mode {
    Select,
    InsertShape,
    ApplyMaterials,
}

#[derive(PartialEq, Clone, Debug)]
pub enum Perspective {
    Top,
    Iso,
}

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

    pub curr_mode           : Mode,
    pub curr_perspective    : Perspective,
    pub curr_shape          : usize,

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

            color_widget    : [83, 83, 83, 255],
            color_selected  : [135, 135, 135, 255],
            color_text      : [244, 244, 244, 255],
            color_text_disabled : [100, 100, 100, 255],

            curr_mode       : Mode::InsertShape,
            curr_shape      : 0,

            curr_perspective: Perspective::Iso,

            fonts,
            code_editor_font,
            ui_font,

            icons,

            cmd             : None
        }
    }
}