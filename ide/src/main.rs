#![deny(clippy::all)]

use ide_lib::prelude::*;

use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use tao::{
    accelerator::{Accelerator, SysMods},
    dpi::PhysicalPosition,
    dpi::LogicalSize,
    event::{Event, DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    menu::{AboutMetadata, MenuBar as Menu, MenuItem, MenuItemAttributes, MenuType},
    clipboard::Clipboard,
    window::WindowBuilder,
    keyboard::Key,
    keyboard::KeyCode,
};

use std::ffi::{CStr, CString};

fn main() -> Result<(), Error> {

    let mut width     : usize = 1068;
    let mut height    : usize = 700;

    env_logger::init();

    // Menus

    let mut clipboard = Clipboard::new();

    let mut first_menu = Menu::new();
    first_menu.add_native_item(MenuItem::About(
        "Forged Thoughts".into(),
        AboutMetadata {
            version: Some("0.1.0".into()),
            ..Default::default()
        },
    ));
    first_menu.add_native_item(MenuItem::Quit);

    let mut file_menu = Menu::new();
    let open_menu_item = file_menu.add_item(
        MenuItemAttributes::new("Open")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyO)),
    );
    file_menu.add_native_item(MenuItem::Separator);
    let save_menu_item = file_menu.add_item(
        MenuItemAttributes::new("Save")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyS)),
    );
    let save_as_menu_item = file_menu.add_item(
        MenuItemAttributes::new("Save As...")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyS)),
    );
    let save_image_as_menu_item = file_menu.add_item(
        MenuItemAttributes::new("Save Image As...")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyI)),
    );

    let mut edit_menu = Menu::new();
    // edit_menu.add_native_item(MenuItem::Undo);
    // edit_menu.add_native_item(MenuItem::Redo);
    // edit_menu.add_native_item(MenuItem::Separator);
    // edit_menu.add_native_item(MenuItem::Cut);
    // edit_menu.add_native_item(MenuItem::Copy);
    // edit_menu.add_native_item(MenuItem::Paste);
    let cut_menu_item = edit_menu.add_item(
        MenuItemAttributes::new("Cut")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyX)),
    );
    let copy_menu_item = edit_menu.add_item(
        MenuItemAttributes::new("Copy")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyC)),
    );
    let paste_menu_item = edit_menu.add_item(
        MenuItemAttributes::new("Paste")
        .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyV)),
    );

    let mut menu_bar = Menu::new();
    menu_bar.add_submenu("&App", true, first_menu);
    menu_bar.add_submenu("&File", true, file_menu);
    menu_bar.add_submenu("&Edit", true, edit_menu);

    // Window

    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(width as f64, height as f64);
        WindowBuilder::new()
            .with_title("Forged Thoughts")
            .with_menu(menu_bar)
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let scale = window.scale_factor() as u32;
        width = (window_size.width / scale) as usize;
        height = (window_size.height / scale) as usize;
        Pixels::new(window_size.width / scale, window_size.height / scale, surface_texture)?
    };

    // Init the code editor

    let mut coords = PhysicalPosition::new(0.0, 0.0);
    let mut is_pressed = false;

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event, .. } => match event {

                // Close events
                WindowEvent::CloseRequested {
                } => {
                    *control_flow = ControlFlow::Exit;
                }

                // Resize the window
                WindowEvent::Resized(size) => {
                    _ = pixels.resize_surface(size.width, size.height);
                    let scale = window.scale_factor() as u32;
                    _ = pixels.resize_buffer(size.width / scale, size.height / scale);
                    width = size.width as usize / scale as usize;
                    height = size.height as usize / scale as usize;
                    window.request_redraw();
                }

                WindowEvent::CursorMoved { position, .. } => {
                    coords = position;
                }

                WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                    logical_key: key,
                    state: ElementState::Pressed,
                    ..
                    },
                ..
                } => {
                    // WARNING: Consider using `key_without_modifers()` if available on your platform.
                    // See the `key_binding` example
                    match key {
                        //Key::Escape => *control_flow = ControlFlow::Exit,
                        Key::Enter => {
                            if ide_lib::rust_special_key_down(KEY_RETURN) {
                                window.request_redraw();
                            }
                        },
                        Key::Backspace => {
                            if ide_lib::rust_special_key_down(KEY_DELETE) {
                                window.request_redraw();
                            }
                        },
                        Key::Delete => {
                            if ide_lib::rust_special_key_down(KEY_DELETE) {
                                window.request_redraw();
                            }
                        },
                        Key::Space => {
                            let key = CString::new(' '.to_string()).unwrap();
                            if ide_lib::rust_key_down(key.as_ptr() as *const i8) {
                                window.request_redraw();
                            }
                        },
                        Key::Tab => {
                            if ide_lib::rust_special_key_down(KEY_TAB) {
                                window.request_redraw();
                            }
                        },
                        Key::Escape => {
                            if ide_lib::rust_special_key_down(KEY_ESCAPE) {
                                window.request_redraw();
                            }
                        },
                        Key::ArrowUp => {
                            if ide_lib::rust_special_key_down(KEY_UP) {
                                window.request_redraw();
                            }
                        },
                        Key::ArrowRight => {
                            if ide_lib::rust_special_key_down(KEY_RIGHT) {
                                window.request_redraw();
                            }
                        },
                        Key::ArrowDown => {
                            if ide_lib::rust_special_key_down(KEY_DOWN) {
                                window.request_redraw();
                            }
                        }
                        Key::ArrowLeft => {
                            if ide_lib::rust_special_key_down(KEY_LEFT) {
                                window.request_redraw();
                            }
                        },
                        Key::Character(char) => {
                            let chars : Vec<char> = char.chars().collect();
                            if chars.len() > 0 {
                                let key = CString::new(char.to_string()).unwrap();
                                if ide_lib::rust_key_down(key.as_ptr() as *const i8) {
                                    window.request_redraw();
                                }
                            }
                        },
                        _ => (),
                    }
                }
                WindowEvent::ModifiersChanged(m) => {
                    if ide_lib::rust_key_modifier_changed(m.shift_key(), m.control_key(), m.alt_key(), m.super_key()) {
                        window.request_redraw();
                    }
                }
                _ => (),
            },

            // Open Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == open_menu_item.clone().id() => {
                ide_lib::rust_open();
            }

            // Save Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == save_menu_item.clone().id() => {
                ide_lib::rust_save();
            }

            // Save As Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == save_as_menu_item.clone().id() => {
                ide_lib::rust_save_as();
            }

            // Save Image As Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == save_image_as_menu_item.clone().id() => {
                ide_lib::rust_save_image_as();
            }

            // Cut Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == cut_menu_item.clone().id() => {
                let string = ide_lib::rust_cut();
                let string_str = unsafe { CStr::from_ptr(string) };
                if let Some(text) = string_str.to_str().ok() {
                    clipboard.write_text(text);
                }
            }

            // Copy Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == copy_menu_item.clone().id() => {
                let string = ide_lib::rust_copy();
                let string_str = unsafe { CStr::from_ptr(string) };
                if let Some(text) = string_str.to_str().ok() {
                    clipboard.write_text(text);
                }
            }

            // Paste Menu Event
            Event::MenuEvent {
                menu_id,
                origin: MenuType::MenuBar,
                ..
            } if menu_id == paste_menu_item.clone().id() => {
                if let Some(text) = clipboard.read_text() {
                    let t = CString::new(text.as_str()).unwrap();
                    ide_lib::rust_paste(t.as_ptr() as *const i8);
                    window.request_redraw();
                }
            }

            // Update internal state and request a redraw
            Event::MainEventsCleared => {
                window.request_redraw();
            }

            // Draw the current frame
            Event::RedrawRequested(_) => {

                let frame = pixels.frame_mut();
                ide_lib::rust_draw(frame.as_mut_ptr(), width as u32, height as u32);

                if pixels
                    .render()
                    .map_err(|e| error!("pixels.render() failed: {}", e))
                    .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                }
            },

            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { /*delta,*/ .. } => {
                    //println!("mouse moved: {:?}", delta),
                    if let Some(pixel_pos) = pixels.window_pos_to_pixel((coords.x as f32, coords.y as f32)).ok() {
                        if is_pressed {
                            if ide_lib::rust_touch_dragged(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                                window.request_redraw();
                            }
                        } else
                        if ide_lib::rust_hover(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                            window.request_redraw();
                        }
                    }
                }
                DeviceEvent::Button {state, .. } => match state {
                    ElementState::Pressed => {
                        //println!("mouse button {} pressed", button);
                        if let Some(pixel_pos) = pixels.window_pos_to_pixel((coords.x as f32, coords.y as f32)).ok() {
                            is_pressed = true;
                            if ide_lib::rust_touch_down(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                                window.request_redraw();
                            }
                        }
                    }
                    ElementState::Released => {
                        //println!("mouse button {} released", button),
                        if let Some(pixel_pos) = pixels.window_pos_to_pixel((coords.x as f32, coords.y as f32)).ok() {
                            is_pressed = false;
                            if ide_lib::rust_touch_up(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                                window.request_redraw();
                            }
                        }
                    }
                    _ => (),
                },

                DeviceEvent::MouseWheel { delta, .. } => match delta {
                    tao::event::MouseScrollDelta::LineDelta(x, y) => {
                        //println!("mouse wheel Line Delta: ({},{})", x, y);
                        if ide_lib::rust_touch_wheel(x * 100.0, y * 100.0) {
                            window.request_redraw();
                        }
                    }
                    tao::event::MouseScrollDelta::PixelDelta(p) => {
                        if ide_lib::rust_touch_wheel(p.x as f32, p.y as f32) {
                            window.request_redraw();
                        }
                    }
                    _ => (),
                },
                _ => (),
            }
            _ => {}
        }
    });
}