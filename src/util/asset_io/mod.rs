use bevy::{asset::{Asset, AssetId, Handle}, color::{Color, ColorToComponents, Srgba}};
use uuid::Uuid;
use bytemuck::Pod;
use futures_lite::{AsyncReadExt, AsyncWriteExt};

pub mod mesh_io;
pub mod image_io;
pub mod standard_material_io;

pub async fn write_string(writer: &mut bevy::asset::io::Writer, counter: &mut usize, string: &String) -> Result<(), std::io::Error>{
    let bytes = string.as_bytes();
    write_u64(bytes.len() as u64, writer, counter).await?;
    dbg!(&string, &counter);
    writer.write_all(bytes).await?;
    *counter += bytes.len();
    Ok(())
}
pub async fn read_string<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<String, std::io::Error>{
    let mut len = read_u64(reader, counter).await?;
    dbg!((len, &counter));
    if len > 255 {
        len = 8;
    }
    let mut string_bytes = vec![0u8; len as usize];
    let bytes = string_bytes.as_mut_slice();
    reader.read_exact(bytes).await?;
    *counter += bytes.len();
    Ok(String::from_utf8(bytes.to_vec()).unwrap())
}

pub async fn write_each<'a, T>(writer: &mut bevy::asset::io::Writer, counter: &mut usize, items: &'a Vec<T>) -> Result<&'a Vec<T>, std::io::Error>{
    dbg!(items.len());
    write_u64(items.len() as u64, writer, counter).await?;
    Ok(items)
}
pub async fn read_each<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<std::ops::Range<usize>, std::io::Error>{
    Ok(0..read_u64(reader, counter).await? as usize)
}

// Length is written first, then each element. read_slice doesnt exist since you cant return a reference
pub async fn write_slice<T: Pod>(writer: &mut bevy::asset::io::Writer, counter: &mut usize, slice: &[T]) -> Result<(), std::io::Error>{
    let bytes = bytemuck::cast_slice::<T, u8>(slice);
    writer.write_all(&bytemuck::cast::<u64, [u8; 8]>(bytes.len() as u64)).await?;
    *counter += 8;
    dbg!(writer.write_all(bytes).await?); // XXX it is fucking insane that writer.write doesn't write all the bytes
    dbg!((bytes.len(), &counter));
    *counter += bytes.len();
    Ok(())
}
pub async fn write_vector<T: Pod>(vec: &Vec<T>, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error> {
    let bytes = bytemuck::cast_slice::<T, u8>(vec.as_slice());
    writer.write_all(bytemuck::cast_slice::<u64, u8>(&[bytes.len() as u64])).await?;
    *counter += 8;
    writer.write_all(bytes).await?;
    *counter += bytes.len();
    Ok(())
}
pub async fn read_vector_casted<'a, T: Pod>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Vec<T>, std::io::Error> {
    let mut len_bytes = [0u8; 8];
    reader.read_exact(&mut len_bytes).await?;
    *counter += 8;
    let vector_len = bytemuck::cast_slice::<u8, u64>(len_bytes.as_slice())[0];
    let mut data_bytes = vec![0u8; vector_len as usize];
    let data_slice = data_bytes.as_mut_slice();
    reader.read_exact(data_slice).await?;
    *counter += data_slice.len();
    Ok(bytemuck::cast_slice::<u8, T>(data_slice).to_vec())
}
pub async fn read_vector<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Vec<u8>, std::io::Error> {
    let mut len_bytes = [0u8; 8];
    reader.read_exact(&mut len_bytes).await?;
    *counter += 8;
    let vector_len = bytemuck::cast_slice::<u8, u64>(len_bytes.as_slice())[0];
    let mut data_bytes = vec![0u8; vector_len as usize];
    let data_slice = data_bytes.as_mut_slice();
    reader.read_exact(data_slice).await?;
    dbg!((vector_len, &counter));
    *counter += data_slice.len();
    Ok(data_slice.to_vec())
}

pub async fn write_option<T>(opt: &Option<T>, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<bool, std::io::Error> {
    if opt.is_some(){
        writer.write_all(&[u8::MAX]).await?;
        *counter += 1;
        Ok(true)
    }else{
        writer.write_all(&[0]).await?;
        *counter += 1;
        Ok(false)
    }
}
pub async fn read_option<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<bool, std::io::Error> {
    let mut read_byte: [u8; 1] = [0; 1];
    reader.read_exact(&mut read_byte).await?;
    *counter += 1;
    Ok(read_byte[0] != 0)
}

pub async fn write_byte(byte: u8, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    writer.write_all(&[byte]).await?; 
    *counter += 1;
    Ok(())
}
pub async fn read_byte<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<u8, std::io::Error>{
    let mut read_byte: [u8; 1] = [0; 1];
    reader.read_exact(&mut read_byte).await?;
    *counter += 1;
    Ok(read_byte[0])
}

pub async fn write_u32(data: u32, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    let bytes = bytemuck::cast::<u32, [u8; 4]>(data);
    writer.write_all(&bytes).await?; 
    *counter += 4;
    Ok(())
}
pub async fn read_u32<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<u32, std::io::Error>{
    let mut bytes: [u8; 4] = [0; 4];
    reader.read_exact(&mut bytes).await?;
    *counter += 4;
    Ok(bytemuck::cast::<[u8; 4], u32>(bytes))
}

pub async fn write_u64(data: u64, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    let bytes = bytemuck::cast::<u64, [u8; 8]>(data);
    writer.write_all(&bytes).await?; 
    *counter += 8;
    Ok(())
}
pub async fn read_u64<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<u64, std::io::Error>{
    let mut bytes: [u8; 8] = [0; 8];
    reader.read_exact(&mut bytes).await?;
    *counter += 8;
    Ok(bytemuck::cast::<[u8; 8], u64>(bytes))
}

pub async fn write_u128(data: u128, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    let bytes = bytemuck::cast::<u128, [u8; 16]>(data);
    writer.write_all(&bytes).await?; 
    *counter += 16;
    Ok(())
}
pub async fn read_u128<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<u128, std::io::Error>{
    let mut bytes: [u8; 16] = [0; 16];
    reader.read_exact(&mut bytes).await?;
    *counter += 16;
    Ok(bytemuck::cast::<[u8; 16], u128>(bytes))
}

pub async fn write_f32(data: f32, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    writer.write_all(&data.to_be_bytes()).await?;
    *counter += 4;
    Ok(())
}
pub async fn read_f32<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<f32, std::io::Error>{
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes).await?;
    *counter += 4;
    Ok(f32::from_be_bytes(bytes))
}

pub async fn write_color(data: Color, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    for val in data.to_srgba().to_f32_array(){write_f32(val, writer, counter).await?;}
    Ok(())
}
pub async fn read_color<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Color, std::io::Error>{
    let mut floats: [f32; 4] = [0.0; 4];
    for i in 0..4{
        floats[i] = read_f32(reader, counter).await?;
    }
    Ok(Srgba::from_f32_array(floats).into())
}

pub async fn write_bool(data: bool, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    if data{writer.write_all(&[u8::MAX]).await?;} else {writer.write_all(&[0u8]).await?;}
    *counter += 1;
    Ok(())
}
pub async fn read_bool<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<bool, std::io::Error>{
    let mut bytes = [0u8];
    reader.read_exact(&mut bytes).await?;
    *counter += 1;
    Ok(bytes[0] != 0)
}

pub async fn write_opt_handle<T: Asset>(data: &Option<&Handle<T>>, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
    if write_option(data, writer, counter).await?{
        match data.unwrap().id(){
            AssetId::Uuid { uuid } => {write_u128(uuid.as_u128(), writer, counter).await?;},
            _ => {}
        }
    }
    Ok(())
}
pub async fn read_opt_handle<'a, T: Asset>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Option<Handle<T>>, std::io::Error>{
    let handle = if read_option(reader, counter).await? {
        let uuid = read_u128(reader, counter).await?;
        Some(Handle::<T>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(uuid) }))
    } else {None};
    Ok(handle)
}
