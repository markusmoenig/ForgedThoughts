use clap::{arg, Command};
use forgedthoughts::{modelbuffer::Vec3, prelude::*};
use std::sync::{Arc, Mutex};

fn cli() -> Command {
    Command::new("ftc")
        .about("Forged Thoughts compiler. Compiles, renders or polygonizes '.ft' node graph files.")
        .author("Markus Moenig")
        .version("0.1.3")
        .subcommand_required(false)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .arg(arg!(<FILE> "The input '.ft' file"))
        .arg_required_else_help(true)
        .subcommand(
            Command::new("render").about("Renders the input to an PNG image. Used by default."), //.arg(arg!(<FILE> "The input file."))
                                                                                                 //.arg_required_else_help(false),
        )
        .subcommand(
            Command::new("polygonize").about("Polygonize the input to an OBJ file."), // .arg(arg!(<VALUE> "The remote to clone"))
                                                                                      // .arg_required_else_help(false),
        )
}

fn main() {
    let matches = cli().get_matches();

    // let mut file_name = "main.ft";
    // let mut polygonize = false;
    let file_name = matches.get_one::<String>("FILE").unwrap();
    // println!("{:?}", matches);

    // #[allow(clippy::single_match)]
    // match matches.subcommand() {
    //     Some(("input", sub_matches)) => {
    //         file_name = sub_matches.get_one::<String>("FILE").expect("required");
    //     }
    //     // Some(("polygonize", _sub_matches)) => {
    //     //     //file_name = sub_matches.get_one::<String>("FILE").expect("required");
    //     //     polygonize = true;
    //     // }
    //     _ => {}
    // }

    println!("{}", file_name);

    let mut ft = FT::new();
    match ft.compile_nodes() {
        Ok(_) => {
            println!("Nodes compiled successfully.");
        }
        Err(err) => {
            println!("Error compiling '{}': ", err);
            return;
        }
    }

    let rc = ft.compile(std::path::PathBuf::new(), file_name.into());
    match rc {
        Ok(_) => {
            println!("Graph compiled successfully.");
        }
        Err(err) => {
            println!("{}", err);
        }
    }

    let ft = Arc::new(ft);

    let width = 600;
    let height = 600;

    let mut buffer = Arc::new(Mutex::new(ft.create_render_buffer(width, height)));
    let rpu = rpu::RPU::new();

    // let wat = ft.nodes.get("Test").unwrap().wat.clone();
    let mut path = std::path::PathBuf::new();
    path.push("out.png");

    let mut model_buffer = ModelBuffer::new([2.1, 2.1, 2.1], 64);
    println!(
        "Model buffer allocated, using {}.",
        model_buffer.memory_usage()
    );
    model_buffer.add_sphere(Vec3::zero(), 1.0, 0);

    let rc = ft.render_2d(Arc::clone(&ft), &rpu, &mut buffer, (60, 60));

    // let rc = ft.render_3d(
    //     Arc::clone(&ft),
    //     &rpu,
    //     &wat,
    //     "main",
    //     &mut buffer,
    //     (60, 60),
    //     Arc::new(model_buffer),
    // );
    // println!("{:?}", rc);

    buffer.lock().unwrap().save(path);

    /*
    let mut file_name = "main.ft";
    let mut polygonize = false;

    match matches.subcommand() {
        Some(("input", sub_matches)) => {
            file_name = sub_matches.get_one::<String>("FILE").expect("required");
        },
        Some(("polygonize", _sub_matches)) => {
                //file_name = sub_matches.get_one::<String>("FILE").expect("required");
            polygonize = true;
        },
        _ => {
        }
    }

    let rc = ft.compile(PathBuf::new(), file_name.into());

    if rc.is_ok() {
        if let Some(mut ctx) = rc.ok() {

            if polygonize == true {

                // Polygonize and save to an OBJ file
                let obj = ft.polygonize(&mut ctx);
                _ = std::fs::write("main.obj", obj);

            } else {

                // Render and save to an PNG file

                let mut buffer = ColorBuffer::new(ctx.settings.width as usize, ctx.settings.height as usize);

                let is_path_tracer = ctx.settings.renderer.iterations > 1;

                for i in 0..ctx.settings.renderer.iterations {

                    let start = get_time();

                    ft.render(&mut ctx, &mut buffer);

                    let t = get_time() - start;

                    if is_path_tracer {
                        println!("Rendered iteration {} in {}ms", i + 1, t);
                    }

                    let out = buffer.to_u8_vec();

                    // Write it to file

                    let path = "main.png";
                    let file = File::create(path).unwrap();
                    let ref mut w = BufWriter::new(file);

                    let mut encoder = png::Encoder::new(w, ctx.settings.width as u32, ctx.settings.height as u32);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    // Adding text chunks to the header
                    encoder
                        .add_text_chunk(
                            "ForgedThughts".to_string(),
                            "This image was generated by ForgedThoughts.com".to_string(),
                        )
                        .unwrap();

                    let mut writer = encoder.write_header().unwrap();

                    writer.write_image_data(&out).unwrap();
                }
            }
        }
    } else
    if let Some(err) = rc.err() {
        println!("{}", err);
    }
    */
}
