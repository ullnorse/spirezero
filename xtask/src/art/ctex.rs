//! Godot 4 compressed textures (`.ctex`, `GST2`): WebP/PNG payloads and the uncompressed and
//! S3TC/BPTC VRAM formats the game ships. Only the first mip level is decoded.

use image::RgbaImage;

const MAGIC: &[u8; 4] = b"GST2";
/// Header up to the embedded image: magic, version, width, height, flags, mipmap limit and three
/// reserved words.
const HEADER: usize = 36;

/// `CompressedTexture2D::DataFormat`.
const DATA_IMAGE: u32 = 0;
const DATA_PNG: u32 = 1;
const DATA_WEBP: u32 = 2;

/// `Image::Format` values handled here.
const FORMAT_RGB8: u32 = 4;
const FORMAT_RGBA8: u32 = 5;
const FORMAT_DXT1: u32 = 17;
const FORMAT_DXT3: u32 = 18;
const FORMAT_DXT5: u32 = 19;
const FORMAT_BPTC_RGBA: u32 = 22;

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

/// Decodes a `.ctex` file.
pub fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    if data.get(..4) != Some(MAGIC) {
        return Err("not a GST2 texture".to_owned());
    }
    let short = || "truncated texture".to_owned();
    let data_format = u32_at(data, HEADER).ok_or_else(short)?;
    let width = u32::from(u16_at(data, HEADER + 4).ok_or_else(short)?);
    let height = u32::from(u16_at(data, HEADER + 6).ok_or_else(short)?);
    let format = u32_at(data, HEADER + 12).ok_or_else(short)?;
    let body = HEADER + 16;
    match data_format {
        DATA_PNG | DATA_WEBP => {
            let len = u32_at(data, body).ok_or_else(short)? as usize;
            let bytes = data.get(body + 4..body + 4 + len).ok_or_else(short)?;
            image::load_from_memory(bytes)
                .map(|i| i.to_rgba8())
                .map_err(|e| format!("embedded image: {e}"))
        }
        DATA_IMAGE => raw(data.get(body..).ok_or_else(short)?, width, height, format),
        other => Err(format!("unsupported texture data format {other}")),
    }
}

/// Decodes an uncompressed or block-compressed first mip level.
fn raw(data: &[u8], width: u32, height: u32, format: u32) -> Result<RgbaImage, String> {
    let (w, h) = (width as usize, height as usize);
    let short = || "truncated texture data".to_owned();
    let rgba: Vec<u8> = match format {
        FORMAT_RGBA8 => data.get(..w * h * 4).ok_or_else(short)?.to_vec(),
        FORMAT_RGB8 => data
            .get(..w * h * 3)
            .ok_or_else(short)?
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        FORMAT_DXT1 | FORMAT_DXT3 | FORMAT_DXT5 | FORMAT_BPTC_RGBA => {
            let mut pixels = vec![0u32; w * h];
            let decoded = match format {
                FORMAT_DXT1 => texture2ddecoder::decode_bc1(data, w, h, &mut pixels),
                FORMAT_DXT3 => texture2ddecoder::decode_bc2(data, w, h, &mut pixels),
                FORMAT_DXT5 => texture2ddecoder::decode_bc3(data, w, h, &mut pixels),
                _ => texture2ddecoder::decode_bc7(data, w, h, &mut pixels),
            };
            decoded.map_err(|e| format!("block decode: {e}"))?;
            // The decoder packs each pixel as 0xAARRGGBB.
            pixels
                .iter()
                .flat_map(|&p| {
                    let [b, g, r, a] = p.to_le_bytes();
                    [r, g, b, a]
                })
                .collect()
        }
        other => return Err(format!("unsupported image format {other}")),
    };
    RgbaImage::from_raw(width, height, rgba).ok_or_else(short)
}
