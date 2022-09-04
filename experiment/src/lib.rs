use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use image::io::Reader as ImageIoReader;
use image::{GenericImageView, ImageBuffer, ImageFormat, Pixel, Rgba, RgbaImage};

use anyhow::{bail, ensure, Context, Result};
use bytemuck::cast_slice;
use c_vec::CVec;
use structopt::StructOpt;
use walkdir::{DirEntry, WalkDir};


// Taken from bench code
fn grayscale_to_rgb(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() * 3);
    for &px in buf {
        for _ in 0..3 {
            out.push(px);
        }
    }
    out
}

fn grayscale_alpha_to_rgba(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(buf.len() * 4);
    for &px in cast_slice::<_, [u8; 2]>(buf) {
        for _ in 0..3 {
            out.push(px[0]);
        }
        out.push(px[1])
    }
    out
}

#[derive(Clone)]
struct Image {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub data: Vec<u8>,
}

impl Image {
    fn read_png(filename: &Path) -> Result<Self> {
        let mut decoder = png::Decoder::new(File::open(filename)?);
        let transformations = png::Transformations::normalize_to_color8();
        decoder.set_transformations(transformations);
        let mut reader = decoder.read_info()?;
        let mut whole_buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut whole_buf)?;
        let buf = &whole_buf[..info.buffer_size()];
        ensure!(info.bit_depth == png::BitDepth::Eight, "invalid bit depth: {:?}", info.bit_depth);
        let (channels, data) = match info.color_type {
            png::ColorType::Grayscale => {
                // png crate doesn't support GRAY_TO_RGB transformation yet
                (3, grayscale_to_rgb(buf))
            }
            png::ColorType::GrayscaleAlpha => {
                // same as above, but with alpha channel
                (4, grayscale_alpha_to_rgba(buf))
            }
            color_type => {
                let channels = color_type.samples();
                ensure!(channels == 3 || channels == 4, "invalid channels: {}", channels);
                (channels as u8, buf[..info.buffer_size()].to_vec())
            }
        };
        Ok(Self { width: info.width, height: info.height, channels, data })
    }

    pub const fn n_pixels(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }

    pub const fn n_bytes(&self) -> usize {
        self.n_pixels() * (self.channels as usize)
    }
}

fn run_compression_to_file(filename: String) {
    let image_filename = format!("{}{}{}", "./", filename, ".png");
    let image = Image::read_png(Path::new(&image_filename)).unwrap();

    let qoi_filename = format!("{}{}{}", "./", filename, ".qoi");
    let qoi_file_path = Path::new(&qoi_filename);
    let qoi_file = File::create(qoi_file_path).unwrap();

    let mut writer = BufWriter::new(qoi_file);
    let mut vec = qoi::encode_to_vec(image.data, image.width, image.height).unwrap();
    writer.write_all(vec.as_mut());

    let file_size = fs::metadata(qoi_file_path).unwrap().len();
    println!("{} qoi filesize: {}", filename, file_size)
}


#[test]
fn test_dark_compression() {
    run_compression_to_file("dark".to_string());
}

#[test]
fn test_dark_line_compression() {
    run_compression_to_file("dark_line".to_string());
}

#[test]
fn test_dark_line2_compression() {
    run_compression_to_file("dark_line2".to_string());
}

fn open_and_decode_image(path: &str) -> Result<RgbaImage> {
    let image = ImageIoReader::open(path)
        .with_context(|| format!("failed to open"))?
        .decode()
        .with_context(|| format!("failed to decode"))?
        .to_rgba8();

    Ok(image)
}

#[test]
fn test_rgba_image() {
    let filename = "dark_line".to_string();
    let image_filename = format!("{}{}{}", "./", filename, ".png");
    let rgba_img = open_and_decode_image(&image_filename).unwrap();
    let rgba_img_u8 = rgba_img.as_raw();

    let qoi_filename = format!("{}{}{}", "./", filename, ".qoi");
    let qoi_file_path = Path::new(&qoi_filename);
    let qoi_file = File::create(qoi_file_path).unwrap();

    let mut writer = BufWriter::new(qoi_file);
    let mut vec = qoi::encode_to_vec(rgba_img_u8, rgba_img.width(), rgba_img.height()).unwrap();
    writer.write_all(vec.as_mut());

    let file_size = fs::metadata(qoi_file_path).unwrap().len();
    println!("{} qoi filesize: {}", filename, file_size)
}

#[test]
fn test_decode() {
    let filename = "dark_line".to_string();
    run_compression_to_file("dark_line".to_string());
    let qoi_filename = format!("{}{}{}", "./", filename, ".qoi");
    let qoi_vec = std::fs::read(qoi_filename).unwrap();

    let (header, pixels, islands) = qoi::decode_qoi(qoi_vec).unwrap();
    let rgba_img = RgbaImage::from_vec(header.width, header.height, pixels).unwrap();
    rgba_img.save("./dark_line_from_qoi.png");
}

#[test]
fn test_decode2() {
    let filename = "dark_line2".to_string();
    run_compression_to_file("dark_line2".to_string());
    let qoi_filename = format!("{}{}{}", "./", filename, ".qoi");
    let qoi_vec = std::fs::read(qoi_filename).unwrap();

    let (header, pixels, islands) = qoi::decode_qoi(qoi_vec).unwrap();
    let rgba_img = RgbaImage::from_vec(header.width, header.height, pixels).unwrap();
    rgba_img.save("./dark_line2_from_qoi.png");
}

#[test]
fn test_decode3() {
    let filename = "4b".to_string();
    run_compression_to_file("4b".to_string());
    let qoi_filename = format!("{}{}{}", "./", filename, ".qoi");
    let qoi_vec = std::fs::read(qoi_filename).unwrap();

    let (header, pixels, islands) = qoi::decode_qoi(qoi_vec).unwrap();
    let rgba_img = RgbaImage::from_vec(header.width, header.height, pixels).unwrap();
    rgba_img.save("./4b_from_qoi.png");
}



#[test]
fn test_islands() {
    let filename = "diff_tmp".to_string();
    let image_filename = format!("{}{}{}", "./", filename, ".png");
    let rgba_img = open_and_decode_image(&image_filename).unwrap();
    let rgba_img_u8 = rgba_img.as_raw();

    qoi::encode_to_vec( rgba_img_u8, rgba_img.width(), rgba_img.height());
}



