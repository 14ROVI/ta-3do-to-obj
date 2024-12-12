use bincode::config::{FixintEncoding, WithOtherIntEncoding};
use bincode::{DefaultOptions, Options};
use clap::Parser;
use lazy_static::lazy_static;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::mem;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    file: String,
}

struct Buffer {
    data: Vec<u8>,
    cursor: usize,
}
impl Buffer {
    fn new(data: Vec<u8>) -> Self {
        Buffer { data, cursor: 0 }
    }

    fn seek(&mut self, index: u32) {
        self.cursor = index as usize;
    }

    fn seek_relative(&mut self, index: i64) {
        self.cursor = (self.cursor as i64 + index) as usize;
    }

    fn read(&self, n_bytes: usize) -> &[u8] {
        &self.data[self.cursor..(self.cursor + n_bytes)]
    }

    fn read_string(&self) -> String {
        let string_len = self.data[self.cursor..]
            .iter()
            .position(|&c| c == b'\0')
            .unwrap();

        String::from_utf8(self.read(string_len).to_vec()).unwrap()
    }
}

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct TagObject {
    version_signature: u32,
    number_of_vertexes: u32,
    number_of_primitives: u32,
    offset_to_selection_primitive: u32,
    x_from_parent: i32,
    y_from_parent: i32,
    z_from_parent: i32,
    offset_to_object_name: u32,
    always_0: u32,
    offset_to_vertex_array: u32,
    offset_to_primitive_array: u32,
    offset_to_sibling_object: u32,
    offset_to_child_object: u32,
}

#[derive(Deserialize, Debug, Copy, Clone)]
struct Offset {
    x: i32,
    y: i32,
    z: i32,
}

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct TagPrimitive {
    color_index: u32,
    number_of_vertex_indexes: u32,
    always_0: u32,
    offset_to_vertex_index_array: u32,
    offset_to_texture_name: u32,
    unknown_1: u32,
    unknown_2: u32,
    is_colored: u32,
}

#[repr(C, packed)]
#[derive(Deserialize, Debug, Copy, Clone)]
struct TagVertex {
    x: i32,
    y: i32,
    z: i32,
}

lazy_static! {
    static ref DECODER: WithOtherIntEncoding<DefaultOptions, FixintEncoding> =
        DefaultOptions::new().with_fixint_encoding();
    static ref SCALE_FACTOR: i32 = 1000;
}

fn read_struct<T: DeserializeOwned + Clone>(buf: &mut Buffer) -> T {
    DECODER.deserialize(buf.read(mem::size_of::<T>())).unwrap()
}

fn read_primatives(buf: &mut Buffer, object: &TagObject) -> Vec<TagPrimitive> {
    let mut primatives = Vec::new();

    buf.seek(object.offset_to_primitive_array.into());

    for _ in 0..object.number_of_primitives {
        primatives.push(read_struct::<TagPrimitive>(buf));
        buf.seek_relative(mem::size_of::<TagPrimitive>() as i64);
    }

    return primatives;
}

fn read_vertexes(buf: &mut Buffer, object: &TagObject) -> Vec<TagVertex> {
    let mut vertexes = Vec::new();

    buf.seek(object.offset_to_vertex_array.into());

    for _ in 0..object.number_of_vertexes {
        vertexes.push(read_struct::<TagVertex>(buf));
        buf.seek_relative(mem::size_of::<TagVertex>() as i64);
    }

    return vertexes;
}

fn traverse(
    buf: &mut Buffer,
    obj_writter: &mut BufWriter<File>,
    object: &TagObject,
    n_verticies_written: &mut u32,
    parent_offset: Offset,
    indent: usize,
) {
    let offset = Offset {
        x: parent_offset.x + (object.x_from_parent as i32),
        y: parent_offset.y + (object.y_from_parent as i32),
        z: parent_offset.z + (object.z_from_parent as i32),
    };

    diplay_data(
        buf,
        obj_writter,
        &object,
        n_verticies_written,
        offset,
        indent,
    );

    // go over children
    if object.offset_to_child_object != 0 {
        buf.seek(object.offset_to_child_object.into());
        let child = read_struct::<TagObject>(buf);

        traverse(
            buf,
            obj_writter,
            &child,
            n_verticies_written,
            offset,
            indent + 1,
        );
    }

    // go over siblings
    if object.offset_to_sibling_object != 0 {
        buf.seek(object.offset_to_sibling_object.into());
        let sibling = read_struct::<TagObject>(buf);

        traverse(
            buf,
            obj_writter,
            &sibling,
            n_verticies_written,
            parent_offset,
            indent + 1,
        );
    }
}

fn diplay_data(
    buf: &mut Buffer,
    obj_writter: &mut BufWriter<File>,
    object: &TagObject,
    n_verticies_written: &mut u32,
    parent_offset: Offset,
    indent: usize,
) {
    buf.seek(object.offset_to_object_name.into());
    let name = buf.read_string();

    // println!("{}{}", " ".repeat(indent * 2), name);
    // println!("{}{:?}", " ".repeat(indent * 2), parent_offset);

    writeln!(obj_writter).unwrap();
    writeln!(obj_writter, "o {}", name).unwrap();

    let vertexes = read_vertexes(buf, object);
    for v in &vertexes {
        let (x, y, z) = (v.x, v.y, v.z);

        writeln!(
            obj_writter,
            "v {} {} {}",
            (parent_offset.x + x) / *SCALE_FACTOR,
            (parent_offset.y + y) / *SCALE_FACTOR,
            (parent_offset.z + z) / *SCALE_FACTOR
        )
        .unwrap();
    }

    let primatives = read_primatives(buf, object);
    for p in primatives {
        write!(obj_writter, "f").unwrap();

        buf.seek(p.offset_to_vertex_index_array.into());
        for _ in 0..p.number_of_vertex_indexes {
            let vertex_index = read_struct::<u16>(buf);
            buf.seek_relative(mem::size_of::<u16>() as i64);

            write!(
                obj_writter,
                " {}",
                *n_verticies_written + (vertex_index as u32) + 1
            )
            .unwrap();
        }

        writeln!(obj_writter).unwrap();
    }

    *n_verticies_written += vertexes.len() as u32;
}

fn main() {
    let args = Args::parse();
    let file_name = args.file.split_terminator(".").next().unwrap();

    let mut buffer = {
        let data = fs::read(file_name.to_owned() + ".3do").unwrap();
        Buffer::new(data)
    };

    let mut obj_writter = {
        let file = File::create(file_name.to_owned() + ".obj").expect("unable to create file");
        BufWriter::new(file)
    };

    let root_object = read_struct::<TagObject>(&mut buffer);
    let mut n_verticies_written = 0;

    traverse(
        &mut buffer,
        &mut obj_writter,
        &root_object,
        &mut n_verticies_written,
        Offset { x: 0, y: 0, z: 0 },
        0,
    );
}