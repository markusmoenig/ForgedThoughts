pub mod editor;
pub mod ui;

use rust_embed::RustEmbed;
#[derive(RustEmbed)]
#[folder = "embedded/"]
#[exclude = ".txt"]
#[exclude = ".DS_Store"]
pub struct Embedded;

pub mod prelude {
    pub use crate::Embedded;

    pub use crate::ui::draw2d::*;
    pub use crate::ui::context::*;
    pub use crate::ui::rect::Rect;

    pub use crate::ui::widgets::{WidgetCmd, Widget};
    pub use crate::ui::widgets::code_toolbar::CodeToolbar;
    pub use crate::ui::widgets::text_button::TextButton;

    pub use code_editor::WidgetKey;

    pub const KEY_ESCAPE        : u32 = 0;
    pub const KEY_RETURN        : u32 = 1;
    pub const KEY_DELETE        : u32 = 2;
    pub const KEY_UP            : u32 = 3;
    pub const KEY_RIGHT         : u32 = 4;
    pub const KEY_DOWN          : u32 = 5;
    pub const KEY_LEFT          : u32 = 6;
    pub const KEY_SPACE         : u32 = 7;
    pub const KEY_TAB           : u32 = 8;
}

use crate::editor::Editor;
use prelude::*;

use std::os::raw::c_char;
use std::ffi::{CStr, CString};

use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref EDITOR: Mutex<Editor> = Mutex::new(Editor::new());
}

#[no_mangle]
pub extern "C" fn rust_draw(pixels: *mut u8, width: u32, height: u32) {
    let length = width as usize * height as usize * 4;
    let slice = unsafe { std::slice::from_raw_parts_mut(pixels, length) };

    EDITOR.lock().unwrap().draw(slice, width as usize, height as usize);
}

#[no_mangle]
pub extern "C" fn rust_target_fps() -> u32 {
    30
}

#[no_mangle]
pub extern "C" fn rust_hover(x: f32, y: f32) -> bool {
    //println!("hover {} {}", x, y);
    EDITOR.lock().unwrap().hover(x, y)
}

#[no_mangle]
pub extern "C" fn rust_touch_down(x: f32, y: f32) -> bool {
    //println!("touch down {} {}", x, y);
    EDITOR.lock().unwrap().touch_down(x, y)
}

#[no_mangle]
pub extern "C" fn rust_touch_dragged(x: f32, y: f32) -> bool {
    //println!("touch dragged {} {}", x, y);
    EDITOR.lock().unwrap().touch_dragged(x, y)
}

#[no_mangle]
pub extern "C" fn rust_touch_up(x: f32, y: f32) -> bool {
    //println!("touch up {} {}", x, y);
    EDITOR.lock().unwrap().touch_up(x, y)
}

#[no_mangle]
pub extern "C" fn rust_touch_wheel(x: f32, y: f32) -> bool {
    //println!("touch up {} {}", x, y);
    EDITOR.lock().unwrap().mouse_wheel((x as isize, y as isize))
}

#[no_mangle]
pub extern "C" fn rust_key_down(p: *const c_char) -> bool {
    let c_str = unsafe { CStr::from_ptr(p) };
    if let Some(key) = c_str.to_str().ok() {
        if let Some(ch ) = key.chars().next() {
            return EDITOR.lock().unwrap().key_down(Some(ch), None);
        }
    }
    false
}

#[no_mangle]
pub extern "C" fn rust_special_key_down(key: u32) -> bool {
    if key == KEY_ESCAPE {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Escape))
    } else
    if key == KEY_RETURN {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Return))
    } else
    if key == KEY_DELETE {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Delete))
    } else
    if key == KEY_UP {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Up))
    } else
    if key == KEY_RIGHT {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Right))
    } else
    if key == KEY_DOWN {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Down))
    } else
    if key == KEY_LEFT {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Left))
    } else
    if key == KEY_SPACE {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Space))
    } else {
    //if key == KEY_TAB {
        EDITOR.lock().unwrap().key_down(None, Some(WidgetKey::Tab))
    }
}

#[no_mangle]
pub extern "C" fn rust_key_modifier_changed(shift: bool, ctrl: bool, alt: bool, logo: bool) -> bool {
    EDITOR.lock().unwrap().modifier_changed(shift, ctrl, alt, logo)
}

#[no_mangle]
pub extern "C" fn rust_dropped_file(p: *const c_char) {
    let path_str = unsafe { CStr::from_ptr(p) };
    if let Some(path) = path_str.to_str().ok() {
        EDITOR.lock().unwrap().dropped_file(path.to_string());
    }
}

#[no_mangle]
pub extern "C" fn rust_open() {
    EDITOR.lock().unwrap().open();
}

#[no_mangle]
pub extern "C" fn rust_save() {
    EDITOR.lock().unwrap().save();
}

#[no_mangle]
pub extern "C" fn rust_save_as() {
    EDITOR.lock().unwrap().save_as();
}

#[no_mangle]
pub extern "C" fn rust_save_image_as() {
    EDITOR.lock().unwrap().save_image_as();
}

#[no_mangle]
pub extern "C" fn rust_cut() -> *mut c_char{
    let text = EDITOR.lock().unwrap().cut();
    CString::new(text).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn rust_copy() -> *mut c_char{
    let text = EDITOR.lock().unwrap().copy();
    CString::new(text).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn rust_paste(p: *const c_char) {
    let text_str = unsafe { CStr::from_ptr(p) };
    if let Some(text) = text_str.to_str().ok() {
        EDITOR.lock().unwrap().paste(text.to_string());
    }
}