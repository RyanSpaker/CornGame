use bevy::{asset::RenderAssetUsages, prelude::*};
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};
use super::*;

/*
    This Code is used to Load and Save Image data in an easy, modular form
*/

/// Saves an Image to the writer with all necessary reading information present in the data. Returns the total number of bytes written.
pub async fn save_image(image: &Image, writer: &mut bevy::asset::io::Writer) -> Result<usize, std::io::Error>{
    let mut byte_counter: usize = 0;
    write_byte(image.asset_usage.bits(), writer, &mut byte_counter).await?;
    write_byte(encode_texture_format(&image.texture_descriptor.format), writer, &mut byte_counter).await?;
    write_byte(encode_texture_dimension(&image.texture_descriptor.dimension), writer, &mut byte_counter).await?;
    write_extent3d(writer, &mut byte_counter, image.texture_descriptor.size).await?;
    write_slice(writer, &mut byte_counter, image.data.as_slice()).await?;
    return Ok(byte_counter);
}
/// Reads an Image from the reader. Returns the total number of bytes read.
pub async fn read_image<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Image, std::io::Error>{
    let usage = RenderAssetUsages::from_bits_truncate(read_byte(reader, counter).await?);
    let format = decode_texture_format(read_byte(reader, counter).await?);
    let dimension = decode_texture_dimension(read_byte(reader, counter).await?);
    let size = read_extent3d(reader, counter).await?;
    let data = read_vector(reader, counter).await?;
    return Ok(Image::new(size, dimension, data, format, usage));
}

async fn write_extent3d(writer: &mut bevy::asset::io::Writer, counter: &mut usize, value: Extent3d) -> Result<(), std::io::Error>{
    write_u32(value.width, writer, counter).await?;
    write_u32(value.height, writer, counter).await?;
    write_u32(value.depth_or_array_layers, writer, counter).await?;
    Ok(())
}
async fn read_extent3d<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Extent3d, std::io::Error>{
    let width = read_u32(reader, counter).await?;
    let height = read_u32(reader, counter).await?;
    let depth = read_u32(reader, counter).await?;
    Ok(Extent3d{width, height, depth_or_array_layers: depth})
}

pub fn encode_texture_format(val: &TextureFormat) -> u8 {
    /* Pretty sure this would also work.. Though you know Texture format is 12 bytes? I think for Astc
    unsafe{ transmute::<TextureFormat, [u8;12]>(*val) }[0];*/

    match *val{
        TextureFormat::R8Unorm => 0,
        TextureFormat::R8Snorm => 1,
        TextureFormat::R8Uint => 2,
        TextureFormat::R8Sint => 3,
        TextureFormat::R16Uint => 4,
        TextureFormat::R16Sint => 5,
        TextureFormat::R16Unorm => 6,
        TextureFormat::R16Snorm => 7,
        TextureFormat::R16Float => 8,
        TextureFormat::Rg8Unorm => 9,
        TextureFormat::Rg8Snorm => 10,
        TextureFormat::Rg8Uint => 11,
        TextureFormat::Rg8Sint => 12,
        TextureFormat::R32Uint => 13,
        TextureFormat::R32Sint => 14,
        TextureFormat::R32Float => 15,
        TextureFormat::Rg16Uint => 16,
        TextureFormat::Rg16Sint => 17,
        TextureFormat::Rg16Unorm => 18,
        TextureFormat::Rg16Snorm => 19,
        TextureFormat::Rg16Float => 20,
        TextureFormat::Rgba8Unorm => 21,
        TextureFormat::Rgba8UnormSrgb => 22,
        TextureFormat::Rgba8Snorm => 23,
        TextureFormat::Rgba8Uint => 24,
        TextureFormat::Rgba8Sint => 25,
        TextureFormat::Bgra8Unorm => 26,
        TextureFormat::Bgra8UnormSrgb => 27,
        TextureFormat::Rgb9e5Ufloat => 28,
        TextureFormat::Rgb10a2Uint => 29,
        TextureFormat::Rgb10a2Unorm => 30,
        TextureFormat::Rg11b10Ufloat => 31,
        TextureFormat::Rg32Uint => 32,
        TextureFormat::Rg32Sint => 33,
        TextureFormat::Rg32Float => 34,
        TextureFormat::Rgba16Uint => 35,
        TextureFormat::Rgba16Sint => 36,
        TextureFormat::Rgba16Unorm => 37,
        TextureFormat::Rgba16Snorm => 38,
        TextureFormat::Rgba16Float => 39,
        TextureFormat::Rgba32Uint => 40,
        TextureFormat::Rgba32Sint => 41,
        TextureFormat::Rgba32Float => 42,
        TextureFormat::Stencil8 => 43,
        TextureFormat::Depth16Unorm => 44,
        TextureFormat::Depth24Plus => 45,
        TextureFormat::Depth24PlusStencil8 => 46,
        TextureFormat::Depth32Float => 47,
        TextureFormat::Depth32FloatStencil8 => 48,
        TextureFormat::NV12 => 49,
        TextureFormat::Bc1RgbaUnorm => 50,
        TextureFormat::Bc1RgbaUnormSrgb => 51,
        TextureFormat::Bc2RgbaUnorm => 52,
        TextureFormat::Bc2RgbaUnormSrgb => 53,
        TextureFormat::Bc3RgbaUnorm => 54,
        TextureFormat::Bc3RgbaUnormSrgb => 55,
        TextureFormat::Bc4RUnorm => 56,
        TextureFormat::Bc4RSnorm => 57,
        TextureFormat::Bc5RgUnorm => 58,
        TextureFormat::Bc5RgSnorm => 59,
        TextureFormat::Bc6hRgbUfloat => 60,
        TextureFormat::Bc6hRgbFloat => 61,
        TextureFormat::Bc7RgbaUnorm => 62,
        TextureFormat::Bc7RgbaUnormSrgb => 63,
        TextureFormat::Etc2Rgb8Unorm => 64,
        TextureFormat::Etc2Rgb8UnormSrgb => 65,
        TextureFormat::Etc2Rgb8A1Unorm => 66,
        TextureFormat::Etc2Rgb8A1UnormSrgb => 67,
        TextureFormat::Etc2Rgba8Unorm => 68,
        TextureFormat::Etc2Rgba8UnormSrgb => 69,
        TextureFormat::EacR11Unorm => 70,
        TextureFormat::EacR11Snorm => 71,
        TextureFormat::EacRg11Unorm => 72,
        TextureFormat::EacRg11Snorm => 73,
        TextureFormat::Astc{block: _, channel: _,} => 74
    }
}
pub fn decode_texture_format(val: u8) -> TextureFormat{
    match val{
        0 => TextureFormat::R8Unorm,
        1 => TextureFormat::R8Snorm,
        2 => TextureFormat::R8Uint,
        3 => TextureFormat::R8Sint,
        4 => TextureFormat::R16Uint,
        5 => TextureFormat::R16Sint,
        6 => TextureFormat::R16Unorm,
        7 => TextureFormat::R16Snorm,
        8 => TextureFormat::R16Float,
        9 => TextureFormat::Rg8Unorm,
        10 => TextureFormat::Rg8Snorm,
        11 => TextureFormat::Rg8Uint,
        12 => TextureFormat::Rg8Sint,
        13 => TextureFormat::R32Uint,
        14 => TextureFormat::R32Sint,
        15 => TextureFormat::R32Float,
        16 => TextureFormat::Rg16Uint,
        17 => TextureFormat::Rg16Sint,
        18 => TextureFormat::Rg16Unorm,
        19 => TextureFormat::Rg16Snorm,
        20 => TextureFormat::Rg16Float,
        21 => TextureFormat::Rgba8Unorm,
        22 => TextureFormat::Rgba8UnormSrgb,
        23 => TextureFormat::Rgba8Snorm,
        24 => TextureFormat::Rgba8Uint,
        25 => TextureFormat::Rgba8Sint,
        26 => TextureFormat::Bgra8Unorm,
        27 => TextureFormat::Bgra8UnormSrgb,
        28 => TextureFormat::Rgb9e5Ufloat,
        29 => TextureFormat::Rgb10a2Uint,
        30 => TextureFormat::Rgb10a2Unorm,
        31 => TextureFormat::Rg11b10Ufloat,
        32 => TextureFormat::Rg32Uint,
        33 => TextureFormat::Rg32Sint,
        34 => TextureFormat::Rg32Float,
        35 => TextureFormat::Rgba16Uint,
        36 => TextureFormat::Rgba16Sint,
        37 => TextureFormat::Rgba16Unorm,
        38 => TextureFormat::Rgba16Snorm,
        39 => TextureFormat::Rgba16Float,
        40 => TextureFormat::Rgba32Uint,
        41 => TextureFormat::Rgba32Sint,
        42 => TextureFormat::Rgba32Float,
        43 => TextureFormat::Stencil8,
        44 => TextureFormat::Depth16Unorm,
        45 => TextureFormat::Depth24Plus,
        46 => TextureFormat::Depth24PlusStencil8,
        47 => TextureFormat::Depth32Float,
        48 => TextureFormat::Depth32FloatStencil8,
        49 => TextureFormat::NV12,
        50 => TextureFormat::Bc1RgbaUnorm,
        51 => TextureFormat::Bc1RgbaUnormSrgb,
        52 => TextureFormat::Bc2RgbaUnorm,
        53 => TextureFormat::Bc2RgbaUnormSrgb,
        54 => TextureFormat::Bc3RgbaUnorm,
        55 => TextureFormat::Bc3RgbaUnormSrgb,
        56 => TextureFormat::Bc4RUnorm,
        57 => TextureFormat::Bc4RSnorm,
        58 => TextureFormat::Bc5RgUnorm,
        59 => TextureFormat::Bc5RgSnorm,
        60 => TextureFormat::Bc6hRgbUfloat,
        61 => TextureFormat::Bc6hRgbFloat,
        62 => TextureFormat::Bc7RgbaUnorm,
        63 => TextureFormat::Bc7RgbaUnormSrgb,
        64 => TextureFormat::Etc2Rgb8Unorm,
        65 => TextureFormat::Etc2Rgb8UnormSrgb,
        66 => TextureFormat::Etc2Rgb8A1Unorm,
        67 => TextureFormat::Etc2Rgb8A1UnormSrgb,
        68 => TextureFormat::Etc2Rgba8Unorm,
        69 => TextureFormat::Etc2Rgba8UnormSrgb,
        70 => TextureFormat::EacR11Unorm,
        71 => TextureFormat::EacR11Snorm,
        72 => TextureFormat::EacRg11Unorm,
        73 => TextureFormat::EacRg11Snorm,
        _ => panic!("Unsupported Texture Format!")
    }
}

pub fn encode_texture_dimension(val: &TextureDimension) -> u8{
    match *val{
        TextureDimension::D1 => 0,
        TextureDimension::D2 => 1,
        TextureDimension::D3 => 2
    }
}
pub fn decode_texture_dimension(val: u8) -> TextureDimension{
    match val{
        0 => TextureDimension::D1,
        1 => TextureDimension::D2,
        2 => TextureDimension::D3,
        _ => panic!("Failed to decode TextureDimension")
    }
}
