extern crate image;
extern crate clap;

use clap::{App, Arg};
use std::path::Path;

mod conversion;

fn main() {
    let matches = App::new("Simple Image Converter")
        .version("0.4.0")
        .author("foresterre <garm@ilumeo.com>")
        .about("Converts an image from one format to another.\n\n\
                Supported input formats are described BMP, GIF, ICO, JPEG, PNG, PPM (limitations may apply). \n\n\
                The image conversion is actually done by the awesome 'image' crate [1]. \n\
                Sic itself is a small command line frontend which supports a small part of the \
                conversion operations supported by the 'image' library. \n\n\
                [1] image crate by PistonDevelopers: https://github.com/PistonDevelopers/image \n\n\
                ")
        .arg(Arg::with_name("forced_output_format")
            .short("f")
            .long("force-format")
            .value_name("FORMAT")
            .help("Output formats supported: JPEG, PNG, GIF, ICO, PPM")
            .takes_value(true))
        .arg(Arg::with_name("input_file")
            .help("Sets the input file")
            .value_name("INPUT_FILE")
            .required(true)
            .index(1))
        .arg(Arg::with_name("output_file")
            .help("Sets the output file")
            .value_name("OUTPUT_FILE")
            .required(true)
            .index(2))
        .get_matches();

    // Can be unwrap because these values are required arguments.
    let input = matches.value_of("input_file").unwrap();
    let output = matches.value_of("output_file").unwrap();
    println!("Provided input file path: {}", input);
    println!("Provided output file path: {}", output);

    let image_buffer: Result<image::DynamicImage, String> =
        image::open(&Path::new(input)).map_err(|err| err.to_string());

    // encode
    let forced_format = matches.value_of("forced_output_format");
    let encode_buffer: Result<(), String> =
        image_buffer.map_err(|err| err.to_string()).and_then(|img| {
            forced_format.map_or_else(
                || conversion::convert_image_unforced(&img, output),
                |format| conversion::convert_image_forced(&img, output, format),
            )
        });

    match encode_buffer {
        Ok(_) => println!("Conversion complete."),
        Err(err) => println!("Conversion ended with an Error: {}", err),
    }
}


