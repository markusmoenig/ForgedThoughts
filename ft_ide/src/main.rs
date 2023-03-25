#![deny(clippy::all)]
#![forbid(unsafe_code)]

use ft_ide_lib::prelude::*;

use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use tao::{
    dpi::PhysicalPosition,
    dpi::LogicalSize,
    event::{Event, DeviceEvent, ElementState, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    menu::{MenuBar, MenuItem},
    window::WindowBuilder,
    keyboard::Key,
};

use std::ffi::CString;

fn main() -> Result<(), Error> {

    let mut width     : usize = 1068;
    let mut height    : usize = 700;

    env_logger::init();
    let event_loop = EventLoop::new();
    let window = {

        let mut file_menu = MenuBar::new();
        file_menu.add_native_item(MenuItem::Quit);

        let mut menu = MenuBar::new();
        menu.add_submenu("File", true, file_menu);
        menu.add_native_item(MenuItem::Quit);

        let size = LogicalSize::new(width as f64, height as f64);
        WindowBuilder::new()
            .with_title("Forged Thoughts IDE")
            .with_menu(menu)
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(width as u32, height as u32, surface_texture)?
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
                            if ft_ide_lib::rust_special_key_down(KEY_RETURN) {
                                window.request_redraw();
                            }
                        },
                        Key::Backspace => {
                            if ft_ide_lib::rust_special_key_down(KEY_DELETE) {
                                window.request_redraw();
                            }
                        },
                        Key::Delete => {
                            if ft_ide_lib::rust_special_key_down(KEY_DELETE) {
                                window.request_redraw();
                            }
                        },
                        Key::Space => {
                            let key = CString::new(' '.to_string()).unwrap();
                            if ft_ide_lib::rust_key_down(key.as_ptr() as *const i8) {
                                window.request_redraw();
                            }
                        },
                        Key::Tab => {
                            if ft_ide_lib::rust_special_key_down(KEY_TAB) {
                                window.request_redraw();
                            }
                        },
                        Key::Escape => {
                            if ft_ide_lib::rust_special_key_down(KEY_ESCAPE) {
                                window.request_redraw();
                            }
                        },
                        Key::ArrowUp => {
                            if ft_ide_lib::rust_special_key_down(KEY_UP) {
                                window.request_redraw();
                            }
                        },
                        Key::ArrowRight => {
                            if ft_ide_lib::rust_special_key_down(KEY_RIGHT) {
                                window.request_redraw();
                            }
                        },
                        Key::ArrowDown => {
                            if ft_ide_lib::rust_special_key_down(KEY_DOWN) {
                                window.request_redraw();
                            }
                        }
                        Key::ArrowLeft => {
                            if ft_ide_lib::rust_special_key_down(KEY_LEFT) {
                                window.request_redraw();
                            }
                        },
                        Key::Character(char) => {
                            let chars : Vec<char> = char.chars().collect();
                            if chars.len() > 0 {
                                let key = CString::new(char.to_string()).unwrap();
                                if ft_ide_lib::rust_key_down(key.as_ptr() as *const i8) {
                                    window.request_redraw();
                                }
                            }
                        },
                        _ => (),
                    }
                }
                WindowEvent::ModifiersChanged(m) => {
                    if ft_ide_lib::rust_key_modifier_changed(m.shift_key(), m.control_key(), m.alt_key(), m.super_key()) {
                        window.request_redraw();
                    }
                }
                _ => (),
            },

            // Update internal state and request a redraw
            Event::MainEventsCleared => {
                window.request_redraw();
            }

            // Draw the current frame
            Event::RedrawRequested(_) => {

                let frame = pixels.frame_mut();
                ft_ide_lib::rust_draw(frame.as_mut_ptr(), width as u32, height as u32);

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
                            if ft_ide_lib::rust_touch_dragged(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                                window.request_redraw();
                            }
                        } else
                        if ft_ide_lib::rust_hover(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                            window.request_redraw();
                        }
                    }
                }
                DeviceEvent::Button {state, .. } => match state {
                    ElementState::Pressed => {
                        //println!("mouse button {} pressed", button);
                        if let Some(pixel_pos) = pixels.window_pos_to_pixel((coords.x as f32, coords.y as f32)).ok() {
                            is_pressed = true;
                            if ft_ide_lib::rust_touch_down(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                                window.request_redraw();
                            }
                        }
                    }
                    ElementState::Released => {
                        //println!("mouse button {} released", button),
                        if let Some(pixel_pos) = pixels.window_pos_to_pixel((coords.x as f32, coords.y as f32)).ok() {
                            is_pressed = false;
                            if ft_ide_lib::rust_touch_up(pixel_pos.0 as f32, pixel_pos.1 as f32) {
                                window.request_redraw();
                            }
                        }
                    }
                    _ => (),
                },

                DeviceEvent::MouseWheel { delta, .. } => match delta {
                    tao::event::MouseScrollDelta::LineDelta(x, y) => {
                        //println!("mouse wheel Line Delta: ({},{})", x, y);
                        if ft_ide_lib::rust_touch_wheel(x * 100.0, y * 100.0) {
                            window.request_redraw();
                        }
                    }
                    tao::event::MouseScrollDelta::PixelDelta(p) => {
                        if ft_ide_lib::rust_touch_wheel(p.x as f32, p.y as f32) {
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