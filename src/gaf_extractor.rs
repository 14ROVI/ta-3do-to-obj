use serde::Deserialize;
use std::fs;
use std::mem::size_of;
extern crate bmp;
use bmp::Image;
use bmp::Pixel;

use crate::palette::PALETTE;
use crate::{read_struct, Buffer};

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct GafHeader {
    version: u32,
    entries: u32,
    unknown_1: u32,
}

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct GafEntry {
    frames: u16,
    unknown_1: u16,
    unknown_2: u32,
    name: [u8; 32],
}

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct GafFrameEntry {
    frame_table_pointer: u32,
    unknown_1: u32,
}

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct GafFrameData {
    width: u16,
    height: u16,
    x_pos: u16,
    y_pos: u16,
    unknown_1: u8,
    compressed: u8,
    frame_pointers: u16,
    unknown_2: u32,
    frame_data_pointer: u32,
    unknown_3: u32,
}

fn read_string(raw: [u8; 32]) -> String {
    let string_len = raw.iter().position(|&c| c == b'\0').unwrap_or(31);

    String::from_utf8(raw[..string_len].to_vec()).unwrap()
}

fn read_image(buf: &mut Buffer, width: u16, height: u16, compressed: u8) -> Image {
    let mut image = Image::new(width.into(), height.into());
    let mut raw = Vec::new();

    if compressed != 0 {
        // we have to uncompress it outself >:(
        for _ in 0..height {
            let line_bytes = read_struct::<u16>(buf);
            buf.seek_relative(size_of::<u16>() as i64);

            for _ in 0..line_bytes {
                let mask = read_struct::<u8>(buf);
                buf.seek_relative(1);

                if (mask & 0x01) == 0x01 {
                    for _ in 0..(mask >> 1) {
                        raw.push(0);
                    }
                } else if (mask & 0x02) == 0x02 {
                    let byte = read_struct::<u8>(buf);
                    buf.seek_relative(1);
                    for _ in 0..((mask >> 2) + 1) {
                        raw.push(byte)
                    }
                } else {
                    for _ in 0..((mask & 0x02) + 1) {
                        let byte = read_struct::<u8>(buf);
                        buf.seek_relative(1);
                        raw.push(byte);
                    }
                }
            }
        }
    }

    if compressed == 0 {
        raw.extend(buf.read((width * height).into()).to_vec());
    }

    for i in 0..(width * height) {
        let byte = raw[i as usize];
        let colour = PALETTE[byte as usize];
        let pixel = Pixel::new(colour[0], colour[1], colour[2]);
        image.set_pixel((i % width).into(), (i / width).into(), pixel);
    }

    return image;
}

fn extract_gaf(buf: &mut Buffer, used_textures: &Vec<String>, extract_folder: &str) {
    let header = read_struct::<GafHeader>(buf);
    buf.seek_relative(size_of::<GafHeader>() as i64);

    let mut entry_pointers = Vec::new();

    for _ in 0..header.entries {
        let entry_pointer = read_struct::<u32>(buf);
        buf.seek_relative(size_of::<u32>() as i64);
        entry_pointers.push(entry_pointer);
    }

    for p in entry_pointers {
        buf.seek(p);
        let entry = read_struct::<GafEntry>(buf);
        let name = read_string(entry.name);

        if used_textures.contains(&name) {
            buf.seek_relative(size_of::<GafEntry>() as i64);
            let frame_entry = read_struct::<GafFrameEntry>(buf);

            buf.seek(frame_entry.frame_table_pointer);
            let mut frame_data = read_struct::<GafFrameData>(buf);

            // we have subframes, just extract the first subframe.
            if frame_data.frame_pointers > 0 {
                buf.seek(frame_data.frame_data_pointer);
                let data_pointer = read_struct::<u32>(buf);
                buf.seek(data_pointer);
                frame_data = read_struct::<GafFrameData>(buf);
            }

            buf.seek(frame_data.frame_data_pointer);
            let image = read_image(
                buf,
                frame_data.width,
                frame_data.height,
                frame_data.compressed,
            );
            let _ = image.save(format!("{}{}.bmp", extract_folder, name));
        }
    }
}

pub fn extract_textures_from_gafs(
    used_textures: &Vec<String>,
    gaf_folder: &str,
    extract_folder: &str,
) {
    if let Ok(gaf_files) = fs::read_dir(gaf_folder) {
        fs::create_dir_all(extract_folder).unwrap();
        for gaf in gaf_files.flatten() {
            let data = fs::read(gaf.path()).unwrap();
            let mut buf = Buffer::new(data);
            extract_gaf(&mut buf, used_textures, extract_folder);
        }
    } else {
        println!("To have textures in your .obj create a folder named gaf_textures in this directory and copy all .gaf files from the game to it.");
    }
}
