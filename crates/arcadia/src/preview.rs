use std::io::Cursor;

use thiserror::Error;

const MAX_WIDTH: u32 = 640;
const MAX_HEIGHT: u32 = 360;

const TEXTURE_NAME: &str = "preview";

#[derive(Debug, Error)]
pub enum PreviewError {
    #[error("could not decode the webp: {0}")]
    Decode(String),
    #[error("could not read the decoded size: {0}")]
    BadSize(String),
    #[error("could not build the bntx file: {0}")]
    Bntx(String),
}

pub struct Preview {
    pub bntx: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn webp_to_bntx(bytes: &[u8]) -> Result<Preview, PreviewError> {
    let (source_width, source_height, pixels) = decode_webp_rgba8(bytes)?;
    let (width, height, pixels) = box_downscale(source_width, source_height, &pixels);
    let bntx = rgba8_to_bntx(width, height, &pixels)?;

    Ok(Preview { bntx, width, height })
}

fn decode_webp_rgba8(webp: &[u8]) -> Result<(u32, u32, Vec<u8>), PreviewError> {
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(webp)).map_err(|err| PreviewError::Decode(err.to_string()))?;
    let (width, height) = decoder.dimensions();
    let size = decoder
        .output_buffer_size()
        .ok_or_else(|| PreviewError::BadSize("decoder didn't return a buffer size".to_string()))?;
    let mut pixels = vec![0u8; size];
    decoder.read_image(&mut pixels).map_err(|err| PreviewError::Decode(err.to_string()))?;

    if !decoder.has_alpha() {
        let mut rgba = vec![255u8; (width as usize) * (height as usize) * 4];
        for (i, chunk) in pixels.chunks_exact(3).enumerate() {
            rgba[i * 4..i * 4 + 3].copy_from_slice(chunk);
        }
        pixels = rgba;
    }

    Ok((width, height, pixels))
}

fn srgb_to_linear_table() -> [f32; 256] {
    let mut table = [0f32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let v = i as f32 / 255.0;
        *slot = if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    }
    table
}

fn linear_to_srgb_table() -> [u8; 4096] {
    let mut table = [0u8; 4096];
    for (i, slot) in table.iter_mut().enumerate() {
        let v = i as f32 / 4095.0;
        let s = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        *slot = (s * 255.0 + 0.5) as u8;
    }
    table
}

fn box_downscale(width: u32, height: u32, rgba8: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut factor = 1u32;
    while width / factor > MAX_WIDTH || height / factor > MAX_HEIGHT {
        factor += 1;
    }

    if factor == 1 {
        return (width, height, rgba8.to_vec());
    }

    let to_linear = srgb_to_linear_table();
    let to_srgb = linear_to_srgb_table();
    let last = (to_srgb.len() - 1) as f32;

    let out_width = (width / factor).max(1);
    let out_height = (height / factor).max(1);
    let mut out = vec![0u8; (out_width as usize) * (out_height as usize) * 4];

    let samples = factor * factor;
    for y in 0..out_height {
        for x in 0..out_width {
            let mut colour = [0f32; 3];
            let mut alpha = 0u32;
            for sy in 0..factor {
                let row = ((y * factor + sy) as usize) * (width as usize);
                for sx in 0..factor {
                    let at = (row + (x * factor + sx) as usize) * 4;
                    for (c, slot) in colour.iter_mut().enumerate() {
                        *slot += to_linear[rgba8[at + c] as usize];
                    }
                    alpha += rgba8[at + 3] as u32;
                }
            }

            let at = ((y as usize) * (out_width as usize) + x as usize) * 4;
            for (c, sum) in colour.iter().enumerate() {
                let lit = (sum / samples as f32).clamp(0.0, 1.0);
                out[at + c] = to_srgb[(lit * last) as usize];
            }
            out[at + 3] = (alpha / samples) as u8;
        }
    }

    (out_width, out_height, out)
}

fn rgba8_to_bntx(width: u32, height: u32, rgba8: &[u8]) -> Result<Vec<u8>, PreviewError> {
    let surface = image_dds::Surface {
        width,
        height,
        depth: 1,
        layers: 1,
        mipmaps: 1,
        image_format: image_dds::ImageFormat::Rgba8UnormSrgb,
        data: rgba8,
    };

    let bntx = bntx::Bntx::from_surface(surface, TEXTURE_NAME).map_err(|err| PreviewError::Bntx(err.to_string()))?;
    let mut out = Cursor::new(Vec::new());
    bntx.write(&mut out).map_err(|err| PreviewError::Bntx(err.to_string()))?;
    Ok(out.into_inner())
}
