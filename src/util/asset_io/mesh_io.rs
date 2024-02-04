use bevy::render::{mesh::{Indices, Mesh, MeshVertexAttribute, MeshVertexAttributeId, VertexAttributeValues}, render_asset::RenderAssetUsages};
use wgpu::{PrimitiveTopology, VertexFormat};
use super::*;

/*
    This Code is used to Load and Save Mesh data in an easy, modular form
*/

/// Saves a Mesh to the writer with all necessary reading information present in the data. Returns the total number of bytes written.
pub async fn save_mesh(mesh: &Mesh, writer: &mut bevy::asset::io::Writer) -> Result<usize, std::io::Error>{
    let mut byte_counter: usize = 0;
    write_byte(encode_primitive_topology(&mesh.primitive_topology()), writer, &mut byte_counter).await?;
    write_byte(mesh.asset_usage.bits(), writer, &mut byte_counter).await?;
    write_indices(&mesh.indices(), writer, &mut byte_counter).await?;
    for (id, attribute) in write_each(writer, &mut byte_counter, &mesh.attributes().collect()).await?.into_iter(){
        write_attribute(writer, &mut byte_counter, id, *attribute).await?;
    }
    //Morph Targets ignored for now. Currently no way to get the Morph Target image handle, so until i need it or the functionality is added, it will remain ignored

    return Ok(byte_counter);
}
/// Reads a Mesh from the reader. Returns the total number of bytes read.
pub async fn read_mesh<'a>(reader: &'a mut bevy::asset::io::Reader::<'a>, byte_counter: &mut usize) -> Result<Mesh, std::io::Error>{
    let primitive_topology = decode_primitive_topology(read_byte(reader, byte_counter).await?);
    let asset_usage = RenderAssetUsages::from_bits_truncate(read_byte(reader, byte_counter).await?);

    let mut mesh = Mesh::new(primitive_topology, asset_usage);
    mesh.set_indices(read_indices(reader, byte_counter).await?);
    
    for _ in read_each(reader, byte_counter).await?{
        let (attr, values) = read_attribute(reader, byte_counter).await?;
        mesh.insert_attribute(attr, values);
    }
    
    //Morph Targets ignored for now. Currently no way to get the Morph Target image handle, so until i need it or the functionality is added, it will remain ignored

    return Ok(mesh);
}

pub async fn write_attribute(writer: &mut bevy::asset::io::Writer, counter: &mut usize, id: &MeshVertexAttributeId, attr: &VertexAttributeValues) -> Result<(), std::io::Error>{
    // Binary search to find id since the usize is private :(
    let mut upper = usize::MAX;
    let mut lower = usize::MIN;
    let mut mid = lower+(upper-lower)/2;
    loop{
        let cur_id = MeshVertexAttribute::new("", mid, wgpu::VertexFormat::Float32).id;
        if *id == cur_id {
            break;
        }else if upper - lower == 1{
            mid = upper;
            break;
        }
        if cur_id > *id{
            upper = mid;
            mid = lower + (upper - lower)/2;
        }else if cur_id < *id {
            lower = mid;
            mid = lower+(upper-lower)/2;
        }
        
    }
    let saved_id: u128 = mid as u128;
    // name:
    let name = if *id == Mesh::ATTRIBUTE_COLOR.id{Mesh::ATTRIBUTE_COLOR.name.to_string()} 
        else if *id == Mesh::ATTRIBUTE_JOINT_INDEX.id{Mesh::ATTRIBUTE_JOINT_INDEX.name.to_string()}
        else if *id == Mesh::ATTRIBUTE_JOINT_WEIGHT.id{Mesh::ATTRIBUTE_JOINT_WEIGHT.name.to_string()}
        else if *id == Mesh::ATTRIBUTE_NORMAL.id{Mesh::ATTRIBUTE_NORMAL.name.to_string()}
        else if *id == Mesh::ATTRIBUTE_POSITION.id{Mesh::ATTRIBUTE_POSITION.name.to_string()}
        else if *id == Mesh::ATTRIBUTE_TANGENT.id{Mesh::ATTRIBUTE_TANGENT.name.to_string()}
        else if *id == Mesh::ATTRIBUTE_UV_0.id{Mesh::ATTRIBUTE_UV_0.name.to_string()}
        else if *id == Mesh::ATTRIBUTE_UV_1.id{Mesh::ATTRIBUTE_UV_1.name.to_string()}
        else {format!("custom_{}", saved_id)};
    
    // format
    let format: VertexFormat = attr.into();
    write_string(writer, counter, &name).await?;
    write_u128(saved_id, writer, counter).await?;
    write_byte(encode_vertex_format(&format), writer, counter).await?;
    // attribute values
    write_slice(writer, counter, attr.get_bytes()).await?;
    Ok(())
}
pub async fn read_attribute<'a>(reader: &'a mut bevy::asset::io::Reader::<'a>, counter: &mut usize) -> Result<(MeshVertexAttribute, VertexAttributeValues), std::io::Error>{
    let name = read_string(reader, counter).await?;
    let id = MeshVertexAttribute::new("", read_u128(reader, counter).await? as usize, VertexFormat::Float16x2).id;
    let format = decode_vertex_format(read_byte(reader, counter).await?);
    let attr = if id == Mesh::ATTRIBUTE_COLOR.id {Mesh::ATTRIBUTE_COLOR}
        else if id == Mesh::ATTRIBUTE_JOINT_INDEX.id {Mesh::ATTRIBUTE_JOINT_INDEX}
        else if id == Mesh::ATTRIBUTE_JOINT_WEIGHT.id {Mesh::ATTRIBUTE_JOINT_WEIGHT}
        else if id == Mesh::ATTRIBUTE_NORMAL.id {Mesh::ATTRIBUTE_NORMAL}
        else if id == Mesh::ATTRIBUTE_POSITION.id {Mesh::ATTRIBUTE_POSITION}
        else if id == Mesh::ATTRIBUTE_TANGENT.id {Mesh::ATTRIBUTE_TANGENT}
        else if id == Mesh::ATTRIBUTE_UV_0.id {Mesh::ATTRIBUTE_UV_0}
        else if id == Mesh::ATTRIBUTE_UV_1.id {Mesh::ATTRIBUTE_UV_1}
        else {MeshVertexAttribute{name: name.clone().leak(), id, format}};
    let value_bytes = read_vector(reader, counter).await?;
    let values = vertex_values_from_fmt(attr.format, value_bytes);
    Ok((attr, values))
}

fn vertex_values_from_fmt(format: VertexFormat, data: Vec<u8>) -> VertexAttributeValues{
    match format{
        VertexFormat::Uint8x2 => VertexAttributeValues::Uint8x2(bytemuck::cast_slice::<u8, [u8; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Uint8x4 => VertexAttributeValues::Uint8x4(bytemuck::cast_slice::<u8, [u8; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Sint8x2 => VertexAttributeValues::Sint8x2(bytemuck::cast_slice::<u8, [i8; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Sint8x4 => VertexAttributeValues::Sint8x4(bytemuck::cast_slice::<u8, [i8; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Unorm8x2 => VertexAttributeValues::Unorm8x2(bytemuck::cast_slice::<u8, [u8; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Unorm8x4 => VertexAttributeValues::Unorm8x4(bytemuck::cast_slice::<u8, [u8; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Snorm8x2 => VertexAttributeValues::Snorm8x2(bytemuck::cast_slice::<u8, [i8; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Snorm8x4 => VertexAttributeValues::Snorm8x4(bytemuck::cast_slice::<u8, [i8; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Uint16x2 => VertexAttributeValues::Uint16x2(bytemuck::cast_slice::<u8, [u16; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Uint16x4 => VertexAttributeValues::Uint16x4(bytemuck::cast_slice::<u8, [u16; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Sint16x2 => VertexAttributeValues::Sint16x2(bytemuck::cast_slice::<u8, [i16; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Sint16x4 => VertexAttributeValues::Sint16x4(bytemuck::cast_slice::<u8, [i16; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Unorm16x2 => VertexAttributeValues::Unorm16x2(bytemuck::cast_slice::<u8, [u16; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Unorm16x4 => VertexAttributeValues::Unorm16x4(bytemuck::cast_slice::<u8, [u16; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Snorm16x2 => VertexAttributeValues::Snorm16x2(bytemuck::cast_slice::<u8, [i16; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Snorm16x4 => VertexAttributeValues::Snorm16x4(bytemuck::cast_slice::<u8, [i16; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Float32 => VertexAttributeValues::Float32(bytemuck::cast_slice::<u8, f32>(data.as_slice()).to_vec()),
        VertexFormat::Float32x2 => VertexAttributeValues::Float32x2(bytemuck::cast_slice::<u8, [f32; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Float32x3 => VertexAttributeValues::Float32x3(bytemuck::cast_slice::<u8, [f32; 3]>(data.as_slice()).to_vec()),
        VertexFormat::Float32x4 => VertexAttributeValues::Float32x4(bytemuck::cast_slice::<u8, [f32; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Uint32 => VertexAttributeValues::Uint32(bytemuck::cast_slice::<u8, u32>(data.as_slice()).to_vec()),
        VertexFormat::Uint32x2 => VertexAttributeValues::Uint32x2(bytemuck::cast_slice::<u8, [u32; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Uint32x3 => VertexAttributeValues::Uint32x3(bytemuck::cast_slice::<u8, [u32; 3]>(data.as_slice()).to_vec()),
        VertexFormat::Uint32x4 => VertexAttributeValues::Uint32x4(bytemuck::cast_slice::<u8, [u32; 4]>(data.as_slice()).to_vec()),
        VertexFormat::Sint32 => VertexAttributeValues::Sint32(bytemuck::cast_slice::<u8, i32>(data.as_slice()).to_vec()),
        VertexFormat::Sint32x2 => VertexAttributeValues::Sint32x2(bytemuck::cast_slice::<u8, [i32; 2]>(data.as_slice()).to_vec()),
        VertexFormat::Sint32x3 => VertexAttributeValues::Sint32x3(bytemuck::cast_slice::<u8, [i32; 3]>(data.as_slice()).to_vec()),
        VertexFormat::Sint32x4 => VertexAttributeValues::Sint32x4(bytemuck::cast_slice::<u8, [i32; 4]>(data.as_slice()).to_vec()),
        _ => panic!("Vertex Format Invalid")
    }
}

pub async fn write_indices(indices: &Option<&Indices>, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error> {
    if write_option(indices, writer, counter).await? {
        write_u64(indices.as_ref().unwrap().len() as u64, writer, counter).await?;
        match indices.as_ref().unwrap(){
            Indices::U16(data) => {
                write_vector(data, writer, counter).await?;
            },
            Indices::U32(data) => {
                write_vector(data, writer, counter).await?;
            }
        }
    }
    Ok(())
}
pub async fn read_indices<'a>(reader: &'a mut bevy::asset::io::Reader::<'a>, counter: &mut usize) -> Result<Option<Indices>, std::io::Error> {
    if read_option(reader, counter).await? {
        let indice_count = read_u64(reader, counter).await? as usize;
        let data = read_vector(reader, counter).await?;
        if data.len() / indice_count == 2 {
            Ok(Some(Indices::U16(bytemuck::cast_slice::<u8, u16>(data.as_slice()).to_vec())))
        }else{
            Ok(Some(Indices::U32(bytemuck::cast_slice::<u8, u32>(data.as_slice()).to_vec())))
        }
    }else {Ok(None)}
}

fn encode_vertex_format(val: &VertexFormat) -> u8{
    match val{
        VertexFormat::Uint8x2 => 0,
        VertexFormat::Uint8x4 => 1,
        VertexFormat::Sint8x2 => 2,
        VertexFormat::Sint8x4 => 3,
        VertexFormat::Unorm8x2 => 4,
        VertexFormat::Unorm8x4 => 5,
        VertexFormat::Snorm8x2 => 6,
        VertexFormat::Snorm8x4 => 7,
        VertexFormat::Uint16x2 => 8,
        VertexFormat::Uint16x4 => 9,
        VertexFormat::Sint16x2 => 10,
        VertexFormat::Sint16x4 => 11,
        VertexFormat::Unorm16x2 => 12,
        VertexFormat::Unorm16x4 => 13,
        VertexFormat::Snorm16x2 => 14,
        VertexFormat::Snorm16x4 => 15,
        VertexFormat::Float16x2 => 16,
        VertexFormat::Float16x4 => 17,
        VertexFormat::Float32 => 18,
        VertexFormat::Float32x2 => 19,
        VertexFormat::Float32x3 => 20,
        VertexFormat::Float32x4 => 21,
        VertexFormat::Uint32 => 22,
        VertexFormat::Uint32x2 => 23,
        VertexFormat::Uint32x3 => 24,
        VertexFormat::Uint32x4 => 25,
        VertexFormat::Sint32 => 26,
        VertexFormat::Sint32x2 => 27,
        VertexFormat::Sint32x3 => 28,
        VertexFormat::Sint32x4 => 29,
        VertexFormat::Float64 => 30,
        VertexFormat::Float64x2 => 31,
        VertexFormat::Float64x3 => 32,
        VertexFormat::Float64x4 => 33,
    }
}
fn decode_vertex_format(val: u8) -> VertexFormat{
    match val{
        0 => VertexFormat::Uint8x2,
        1 => VertexFormat::Uint8x4,
        2 => VertexFormat::Sint8x2,
        3 => VertexFormat::Sint8x4,
        4 => VertexFormat::Unorm8x2,
        5 => VertexFormat::Unorm8x4,
        6 => VertexFormat::Snorm8x2,
        7 => VertexFormat::Snorm8x4,
        8 => VertexFormat::Uint16x2,
        9 => VertexFormat::Uint16x4,
        10 => VertexFormat::Sint16x2,
        11 => VertexFormat::Sint16x4,
        12 => VertexFormat::Unorm16x2,
        13 => VertexFormat::Unorm16x4,
        14 => VertexFormat::Snorm16x2,
        15 => VertexFormat::Snorm16x4,
        16 => VertexFormat::Float16x2,
        17 => VertexFormat::Float16x4,
        18 => VertexFormat::Float32,
        19 => VertexFormat::Float32x2,
        20 => VertexFormat::Float32x3,
        21 => VertexFormat::Float32x4,
        22 => VertexFormat::Uint32,
        23 => VertexFormat::Uint32x2,
        24 => VertexFormat::Uint32x3,
        25 => VertexFormat::Uint32x4,
        26 => VertexFormat::Sint32,
        27 => VertexFormat::Sint32x2,
        28 => VertexFormat::Sint32x3,
        29 => VertexFormat::Sint32x4,
        30 => VertexFormat::Float64,
        31 => VertexFormat::Float64x2,
        32 => VertexFormat::Float64x3,
        _ => VertexFormat::Float64x4,
    }
}

fn encode_primitive_topology(val: &PrimitiveTopology) -> u8 {
    match val {
        PrimitiveTopology::LineList => 0,
        PrimitiveTopology::LineStrip => 1,
        PrimitiveTopology::PointList => 2,
        PrimitiveTopology::TriangleList => 3,
        PrimitiveTopology::TriangleStrip => 4
    }
}
fn decode_primitive_topology(val: u8) -> PrimitiveTopology {
    match val {
        0 => PrimitiveTopology::LineList,
        1 => PrimitiveTopology::LineStrip,
        2 => PrimitiveTopology::PointList,
        3 => PrimitiveTopology::TriangleList,
        _ => PrimitiveTopology::TriangleStrip
    }
}