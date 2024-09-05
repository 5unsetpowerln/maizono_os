use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

fn main() {
    let mut font_8_16 = HashMap::<u16, [u8; 16]>::new();
    let ascii = true;

    let font_path = PathBuf::from_str("./Tamzen8x16r.bdf").unwrap();
    let output_path = "./Tamzen8x16r.rsfont";

    let font_file = File::open(font_path).unwrap();
    let reader = BufReader::new(font_file);
    let mut scanning = false;
    let mut scanning_fades = 0;
    let mut scanning_code = None;

    for line_result in reader.lines() {
        let line = line_result.unwrap();
        if line.contains("STARTCHAR") {
            let code_str = line.split("U+").collect::<Vec<&str>>()[1];
            let code_vec = hex::decode(code_str).unwrap();
            let code = 0x100 * code_vec[0] as u16 + code_vec[1] as u16;
            let vector = [0; 16];
            if ascii && (code >= 0x20 && code <= 0x7e) {
                font_8_16.insert(code, vector);
                scanning_code = Some(code);
            } else {
                scanning_code = None;
            }
            continue;
        }
        if line.contains("BBX") {
            let bbx = line.split(' ').collect::<Vec<&str>>();
            let height = usize::from_str(bbx[2]).unwrap();
            if height == 16 {
                continue;
            }
            if height == 17 {
                println!("{}", scanning_code.unwrap());
                continue;
            }
        }
        if line.contains("BITMAP") {
            scanning = true;
            continue;
        }
        if line.contains("ENDCHAR") {
            scanning_fades = 0;
            scanning = false;
            continue;
        }
        if scanning {
            if let Some(code) = scanning_code {
                let byte_vec = hex::decode(line).unwrap();
                let byte = byte_vec[0];
                let vector = font_8_16.get_mut(&code).unwrap();
                vector[scanning_fades] = byte;
                scanning_fades += 1;
            }
            continue;
        }
    }

    let mut font_8_16_vec = Vec::new();

    for i in 0x20..=0x7e {
        let key = i as u16;
        font_8_16_vec.push(font_8_16.get(&key).unwrap());
    }

    let output_vec = format!(
        "const TAMZEN_FONT: [[u8;16];{}] = {:#?};",
        0x7e - 0x20 + 1,
        font_8_16_vec
    );

    let output_file = File::create(output_path).unwrap();
    let mut output_writer = BufWriter::new(output_file);
    output_writer.write_all(output_vec.as_bytes()).unwrap();
}
