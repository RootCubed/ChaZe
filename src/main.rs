use cairo::{Context, Format, ImageSurface};
use clap::Parser;
use std::fs;
mod osmpbf;
mod render;
mod style_file;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    input: String,

    #[arg(short, long)]
    style_file: String,

    #[arg(short, long, default_value_t = String::from("output.png"))]
    output: String,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    let file = fs::read(args.input).expect("Unable to read file.");
    println!("Reading OSM file...");
    let osm_file = osmpbf::read_osm_file(&file).unwrap();

    let style_file = fs::read_to_string(args.style_file).expect("Unable to read style file.");

    let config = style_file::style::StyleParser::new()
        .parse(&style_file)
        .unwrap();
    println!("{:?}", config);

    let surface = ImageSurface::create(
        Format::ARgb32,
        config.meta.width_pixels(),
        config.meta.height_pixels(),
    )
    .expect("Can't create surface");

    let cr = Context::new(&surface).expect("Can't create context");
    cr.set_source_rgb(0.0, 0.0, 0.0);
    let _ = cr.paint().unwrap();

    let mut renderer = render::Renderer::new(&config.meta, &cr, &osm_file);

    renderer.init();

    for layer in config.layers {
        println!("Rendering layer: {}", layer.name);
        renderer.paint(&layer.commands);
    }

    let mut file = fs::File::create(args.output).expect("Unable to create file");
    surface
        .write_to_png(&mut file)
        .expect("Can't write to file");
}
