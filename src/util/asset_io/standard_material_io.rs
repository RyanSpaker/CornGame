use bevy::{pbr::{OpaqueRendererMethod, ParallaxMappingMethod, StandardMaterial}, prelude::AlphaMode};
use wgpu_types::Face;
use super::*;

/*
    This Code is used to Load and Save Mesh data in an easy, modular form
*/

/// Saves a STandardMaterial to the writer with all necessary reading information present in the data. Returns the total number of bytes written.
pub async fn save_standard_material(material: &StandardMaterial, writer: &mut bevy::asset::io::Writer) -> Result<usize, std::io::Error>{
    let mut byte_counter: usize = 0;
    write_f32(material.perceptual_roughness, writer, &mut byte_counter).await?;
    write_f32(material.metallic, writer, &mut byte_counter).await?;
    write_f32(material.reflectance, writer, &mut byte_counter).await?;
    write_f32(material.diffuse_transmission, writer, &mut byte_counter).await?;
    write_f32(material.specular_transmission, writer, &mut byte_counter).await?;
    write_f32(material.thickness, writer, &mut byte_counter).await?;
    write_f32(material.ior, writer, &mut byte_counter).await?;
    write_f32(material.attenuation_distance, writer, &mut byte_counter).await?;
    write_f32(material.depth_bias, writer, &mut byte_counter).await?;
    write_f32(material.parallax_depth_scale, writer, &mut byte_counter).await?;
    write_f32(material.max_parallax_layer_count, writer, &mut byte_counter).await?;
    write_f32(material.lightmap_exposure, writer, &mut byte_counter).await?;
    write_byte(material.deferred_lighting_pass_id, writer, &mut byte_counter).await?;
    write_color(material.base_color, writer, &mut byte_counter).await?;
    write_color(material.emissive.into(), writer, &mut byte_counter).await?;
    write_color(material.attenuation_color, writer, &mut byte_counter).await?;
    write_bool(material.flip_normal_map_y, writer, &mut byte_counter).await?;
    write_bool(material.double_sided, writer, &mut byte_counter).await?;
    write_bool(material.unlit, writer, &mut byte_counter).await?;
    write_bool(material.fog_enabled, writer, &mut byte_counter).await?;
    if write_option(&material.cull_mode, writer, &mut byte_counter).await?{
        write_byte(encode_face(material.cull_mode.as_ref().unwrap()), writer, &mut byte_counter).await?;
    }
    write_byte(encode_alpha_mode(&material.alpha_mode), writer, &mut byte_counter).await?;
    write_u32(encode_parallax_mapping_method(&material.parallax_mapping_method), writer, &mut byte_counter).await?;
    write_byte(encode_opaque_render_method(&material.opaque_render_method), writer, &mut byte_counter).await?;
    write_opt_handle(&material.base_color_texture.as_ref(), writer, &mut byte_counter).await?;
    write_opt_handle(&material.emissive_texture.as_ref(), writer, &mut byte_counter).await?;
    write_opt_handle(&material.metallic_roughness_texture.as_ref(), writer, &mut byte_counter).await?;
    write_opt_handle(&material.normal_map_texture.as_ref(), writer, &mut byte_counter).await?;
    write_opt_handle(&material.occlusion_texture.as_ref(), writer, &mut byte_counter).await?;
    write_opt_handle(&material.depth_map.as_ref(), writer, &mut byte_counter).await?;
    return Ok(byte_counter);
}
/// Reads a StandardMaterial from the reader. Returns the total number of bytes read.
pub async fn read_standard_material<'a>(reader: &'a mut dyn  bevy::asset::io::Reader, counter: &mut usize) -> Result<StandardMaterial, std::io::Error>{
    let perceptual_roughness = read_f32(reader, counter).await?;
    let metallic = read_f32(reader, counter).await?;
    let reflectance = read_f32(reader, counter).await?;
    let diffuse_transmission = read_f32(reader, counter).await?;
    let specular_transmission = read_f32(reader, counter).await?;
    let thickness = read_f32(reader, counter).await?;
    let ior = read_f32(reader, counter).await?;
    let attenuation_distance = read_f32(reader, counter).await?;
    let depth_bias = read_f32(reader, counter).await?;
    let parallax_depth_scale = read_f32(reader, counter).await?;
    let max_parallax_layer_count = read_f32(reader, counter).await?;
    let lightmap_exposure = read_f32(reader, counter).await?;
    let deferred_lighting_pass_id = read_byte(reader, counter).await?;
    let base_color = read_color(reader, counter).await?;
    let emissive = read_color(reader, counter).await?;
    let attenuation_color = read_color(reader, counter).await?;
    let flip_normal_map_y = read_bool(reader, counter).await?;
    let double_sided = read_bool(reader, counter).await?;
    let unlit = read_bool(reader, counter).await?;
    let fog_enabled = read_bool(reader, counter).await?;
    let cull_mode = if read_option(reader, counter).await? {
        Some(decode_face(read_byte(reader, counter).await?))
    } else {None};
    let alpha_mode = decode_alpha_mode(read_byte(reader, counter).await?);
    let parallax_mapping_method = decode_parallax_mapping_method(read_u32(reader, counter).await?);
    let opaque_render_method = decode_opaque_render_method(read_byte(reader, counter).await?);
    let base_color_texture = read_opt_handle(reader, counter).await?;
    let emissive_texture = read_opt_handle(reader, counter).await?;
    let metallic_roughness_texture = read_opt_handle(reader, counter).await?;
    let normal_map_texture = read_opt_handle(reader, counter).await?;
    let occlusion_texture = read_opt_handle(reader, counter).await?;
    let depth_map = read_opt_handle(reader, counter).await?;
    return Ok(StandardMaterial{ 
        base_color, 
        base_color_texture, 
        emissive: emissive.to_linear(), 
        emissive_texture, 
        perceptual_roughness, 
        metallic, 
        metallic_roughness_texture, 
        reflectance, 
        diffuse_transmission, 
        specular_transmission, 
        thickness, 
        ior, 
        attenuation_distance, 
        attenuation_color, 
        normal_map_texture, 
        flip_normal_map_y, 
        occlusion_texture, 
        double_sided, 
        cull_mode, 
        unlit, 
        fog_enabled, 
        alpha_mode, 
        depth_bias, 
        depth_map, 
        parallax_depth_scale, 
        parallax_mapping_method, 
        max_parallax_layer_count, 
        lightmap_exposure, 
        opaque_render_method, 
        deferred_lighting_pass_id,
        ..Default::default() // XXX definately wrong
    });
}

fn encode_face(val: &Face) -> u8{
    match *val{
        Face::Back => 0,
        Face::Front => 1
    }
}
fn decode_face(val: u8) -> Face{
    match val{
        0 => Face::Back,
        _ => Face::Front
    }
}

fn encode_alpha_mode(val: &AlphaMode) -> u8{
    match *val{
        AlphaMode::Opaque => 0,
        AlphaMode::Blend => 1,
        AlphaMode::Premultiplied => 2,
        AlphaMode::Add => 3,
        AlphaMode::Multiply => 4,
        _ => panic!("Cant Encode Mask Alpha Mode")
    }
}
fn decode_alpha_mode(val: u8) -> AlphaMode{
    match val{
        0 => AlphaMode::Opaque,
        1 => AlphaMode::Blend,
        2 => AlphaMode::Premultiplied,
        3 => AlphaMode::Add,
        4 => AlphaMode::Multiply,
        _ => panic!("Cant decode Mask Alpha Mode")
    }
}

fn encode_parallax_mapping_method(val: &ParallaxMappingMethod) -> u32{
    match *val{
        ParallaxMappingMethod::Occlusion => 0,
        ParallaxMappingMethod::Relief { max_steps } => max_steps + 1,
    }
}
fn decode_parallax_mapping_method(val: u32) -> ParallaxMappingMethod{
    match val{
        0 => ParallaxMappingMethod::Occlusion,
        default => ParallaxMappingMethod::Relief { max_steps: default-1}
    }
}

fn encode_opaque_render_method(val: &OpaqueRendererMethod) -> u8{
    match *val{
        OpaqueRendererMethod::Auto => 0,
        OpaqueRendererMethod::Deferred => 1,
        OpaqueRendererMethod::Forward => 2
    }
}
fn decode_opaque_render_method(val: u8) -> OpaqueRendererMethod{
    match val{
        0 => OpaqueRendererMethod::Auto,
        1 => OpaqueRendererMethod::Deferred,
        _ => OpaqueRendererMethod::Forward
    }
}
