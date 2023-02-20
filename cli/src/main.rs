
use forgedthoughts::prelude::*;

fn main() {

    let file_name = "image.ft";
    let mut code = "".to_string();
    if let Some(input) = std::fs::read_to_string(file_name).ok() {
        code = input;
    }

    let ft = FT::new();

    let rc = ft.compile(code);

    if rc.is_ok() {
        if let Some(scope) = rc.ok() {

            let mut buffer = ColorBuffer::new(800, 600, 0.0);
            ft.render(scope, &mut buffer);
        }
    }
}
