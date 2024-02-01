#![allow(dead_code)]

use std::convert::Infallible;
use bevy::{
    asset::{processor::LoadTransformAndSave, saver::AssetSaver, transformer::{AssetTransformer, TransformedAsset}, AssetLoader}, gltf::{Gltf, GltfLoader, GltfMesh}, pbr::OpaqueRendererMethod, prelude::*, render::{mesh::{Indices, MeshVertexAttribute, MeshVertexAttributeId, VertexAttributeValues}, render_asset::RenderAssetUsages, texture::{ImageAddressMode, ImageFilterMode, ImageSampler}}, utils::{hashbrown::HashMap, Uuid}
};
use futures_lite::AsyncWriteExt;
use wgpu::{Face, PrimitiveTopology, TextureViewDimension, VertexFormat};

pub struct CornAssetPlugin;
impl Plugin for CornAssetPlugin{
    fn build(&self, app: &mut App) {
        app.register_asset_processor::<LoadTransformAndSave<GltfLoader, CornAssetTransformer, CornAssetSaver>>(LoadTransformAndSave::new(CornAssetTransformer, CornAssetSaver));
        app.add_systems(Startup, test);
        app.register_asset_loader::<CornAssetLoader>(CornAssetLoader);
        app.init_asset::<RawCornAsset>();
    }
}

pub fn test(assets: Res<AssetServer>){
    let _temp: Handle<RawCornAsset> = assets.load("models/Corn.gltf");
}


#[derive(Debug, Clone, TypePath, Asset)]
pub struct RawCornAsset{
    master_mesh: Mesh,
    lod_data: Vec<CornMeshLod>,
    materials: Vec<StandardMaterial>,
    textures: Vec<Image>
}

#[derive(Default, Debug, Clone)]
pub struct CornMeshLod{
    start_vertex: usize,
    total_vertices: usize,
    /// Vertex Count, Material Index
    sub_mesh_data: Vec<(usize, usize)>,
}
impl CornMeshLod{
    pub fn from_start(start_vertex: usize) -> Self{ Self{start_vertex, total_vertices: 0, sub_mesh_data: vec![]} }
}


pub struct CornAssetTransformer;
impl AssetTransformer for CornAssetTransformer{
    type AssetInput = Gltf;

    type AssetOutput = RawCornAsset;

    type Settings = ();

    type Error = Infallible;

    fn transform<'a>(
        &'a self,
        mut asset: TransformedAsset<Self::AssetInput>,
        _settings: &'a Self::Settings,
    ) -> bevy::utils::BoxedFuture<'a, Result<TransformedAsset<Self::AssetOutput>, Self::Error>> {
        Box::pin(async move {
            /*
                This Code is responsible for 
                - taking in a GLTF asset,
                - Extracting the important data from said asset
                - Merging all corn meshes from all LOD's into a single master mesh
                - Saving the materials used by each part of the corn
                - Saving any textures the materials use
                - Replacing the image handles in the materials to handles wrapping an index into the saved image array
                - Saving mesh data such as vertex count for indirect buffer's
             */
            let mut gltf_mesh_labels: HashMap<Handle<GltfMesh>, String> = HashMap::default();
            let mut mesh_labels: HashMap<Handle<Mesh>, String> = HashMap::default();
            let mut material_labels: HashMap<Handle<StandardMaterial>, String> = HashMap::default();
            let mut texture_labels: HashMap<Handle<Image>, String> = HashMap::default();
            let labels: Vec<&str> = asset.iter_labels().collect();
            for label in labels{
                if let Some(handle) = asset.get_handle::<str, GltfMesh>(label){
                    gltf_mesh_labels.insert(handle, label.to_string());
                }
                if let Some(handle) = asset.get_handle::<str, Mesh>(label){
                    mesh_labels.insert(handle, label.to_string());
                }
                if let Some(handle) = asset.get_handle::<str, StandardMaterial>(label){
                    material_labels.insert(handle, label.to_string());
                }
                if let Some(handle) = asset.get_handle::<str, Image>(label){
                    texture_labels.insert(handle, label.to_string());
                }
            }
            // Handle -> Label, Name
            let mut named_meshes: HashMap<Handle<GltfMesh>, (String, String)> = HashMap::default();
            for (name, handle) in asset.named_meshes.iter(){
                named_meshes.insert(handle.to_owned(), (gltf_mesh_labels.get(handle).unwrap().to_owned(), name.to_owned()));
            }
            // List of Materials used by the corn mesh
            let mut used_materials: Vec<Handle<StandardMaterial>> = vec![];
            // List of Lods each a list of meshes and the index of the material that mesh uses
            let mut lods: Vec<Vec<(Handle<Mesh>, usize)>> = vec![];
            for (_, (label, name)) in named_meshes.iter(){
                if name[..7] != *"CornLOD" {continue;}
                let sub_asset = asset.get_labeled::<GltfMesh, str>(label).unwrap();
                let gltf_mesh = sub_asset.get();
                let primitive = &gltf_mesh.primitives[0];
                let mat =  primitive.material.as_ref().unwrap().to_owned();
                let mesh = primitive.mesh.to_owned();
                let pos = if let Some(pos) = used_materials.iter().position(|e| *e == mat) {pos} else {
                    used_materials.push(mat.to_owned());
                    used_materials.len() - 1
                };
                let lod_level = name[7..8].parse::<usize>().unwrap();
                let mesh_index = if name.len() == 8 {0} else {name[11..12].parse::<usize>().unwrap()};
                while lods.len() <= lod_level {lods.push(vec![]);}
                while lods[lod_level].len() <= mesh_index {lods[lod_level].push((Handle::default(), 0));}
                lods[lod_level][mesh_index] = (mesh, pos);
            }
            // Label, mat_index
            let lods: Vec<Vec<(String, usize)>> = lods.into_iter().map(|lod| lod.into_iter().map(|(handle, mat)| 
                (mesh_labels.get(&handle).unwrap().to_owned(), mat)
            ).collect()).collect();

            let mut used_materials: Vec<StandardMaterial> = used_materials.into_iter().map(|handle| {
                let label = material_labels.get(&handle).unwrap();
                let transformed = asset.get_labeled::<StandardMaterial, str>(label.as_str()).unwrap();
                transformed.get().clone()
            }).collect();

            let (master_mesh, vertex_counts) = combine_corn_mesh(lods, &mut asset).await;

            let mut used_images: Vec<Handle<Image>> = vec![];
            for mat in used_materials.iter_mut(){
                if let Some(handle) = mat.base_color_texture.clone() {
                    let pos = if let Some(pos) = used_images.iter().position(|e| *e == handle) {pos} else {
                        used_images.push(handle.to_owned());
                        used_images.len() - 1
                    };
                    mat.base_color_texture = Some(Handle::<Image>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(pos as u128) }));
                }
                if let Some(handle) = mat.depth_map.clone() {
                    let pos = if let Some(pos) = used_images.iter().position(|e| *e == handle) {pos} else {
                        used_images.push(handle.to_owned());
                        used_images.len() - 1
                    };
                    mat.depth_map = Some(Handle::<Image>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(pos as u128) }));
                }
                if let Some(handle) = mat.emissive_texture.clone() {
                    let pos = if let Some(pos) = used_images.iter().position(|e| *e == handle) {pos} else {
                        used_images.push(handle.to_owned());
                        used_images.len() - 1
                    };
                    mat.emissive_texture = Some(Handle::<Image>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(pos as u128) }));
                }
                if let Some(handle) = mat.metallic_roughness_texture.clone() {
                    let pos = if let Some(pos) = used_images.iter().position(|e| *e == handle) {pos} else {
                        used_images.push(handle.to_owned());
                        used_images.len() - 1
                    };
                    mat.metallic_roughness_texture = Some(Handle::<Image>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(pos as u128) }));
                }
                if let Some(handle) = mat.normal_map_texture.clone() {
                    let pos = if let Some(pos) = used_images.iter().position(|e| *e == handle) {pos} else {
                        used_images.push(handle.to_owned());
                        used_images.len() - 1
                    };
                    mat.normal_map_texture = Some(Handle::<Image>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(pos as u128) }));
                }
                if let Some(handle) = mat.occlusion_texture.clone() {
                    let pos = if let Some(pos) = used_images.iter().position(|e| *e == handle) {pos} else {
                        used_images.push(handle.to_owned());
                        used_images.len() - 1
                    };
                    mat.occlusion_texture = Some(Handle::<Image>::Weak(AssetId::Uuid { uuid: Uuid::from_u128(pos as u128) }));
                }
            }
            
            let used_images: Vec<Image> = used_images.into_iter().map(|handle| {
                let transform = asset.get_labeled::<Image, str>(texture_labels.get(&handle).unwrap()).unwrap();
                transform.get().clone()
            }).collect();

            return Ok(asset.replace_asset(RawCornAsset{
                master_mesh, lod_data: vertex_counts, materials: used_materials, textures: used_images
            }));
        })
    }
}

/// Given a vector of lods, each a vector of Mesh Label, material index, Combine the meshes into a single master mesh, and track vertex information, returning both.
async fn combine_corn_mesh(lods: Vec<Vec<(String, usize)>>, asset: &mut TransformedAsset<Gltf>) -> (Mesh, Vec<CornMeshLod>){
    let mut master_mesh = Mesh::new(wgpu::PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    let mut vertex_counts: Vec<CornMeshLod> = vec![];

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normal: Vec<[f32; 3]> = Vec::new();
    let mut uv: Vec<[f32; 2]> = Vec::new();
    let mut tangent: Vec<[f32; 4]> = Vec::new();

    let mut materials: Vec<u32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut indices_offset: u32 = 0;
    for lod in lods.iter(){
        vertex_counts.push(CornMeshLod::from_start(indices.len()));
        for (mesh_label, mat) in lod.iter(){
            let transformed = asset.get_labeled::<Mesh, str>(mesh_label.as_str()).unwrap();
            let mesh = transformed.get();
            if let Some(Indices::U16(mesh_indices)) = mesh.indices(){
                indices.extend(mesh_indices.iter().map(|index| *index as u32+indices_offset));
                if let Some(VertexAttributeValues::Float32x3(vertex_positions)) = 
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                {
                    positions.extend(vertex_positions);
                    if let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL){
                        normal.extend(normals);
                    }
                    if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0){
                        uv.extend(uvs);
                    }
                    if let Some(VertexAttributeValues::Float32x4(tangents)) = mesh.attribute(Mesh::ATTRIBUTE_TANGENT){
                        tangent.extend(tangents);
                    }
                    materials.extend([*mat as u32].repeat(vertex_positions.len()));
                    indices_offset += vertex_positions.len() as u32;
                }
                vertex_counts.last_mut().map(|val| val.sub_mesh_data.push((mesh_indices.len(), *mat)));
            }
        }
        vertex_counts.last_mut().map(|val| {val.total_vertices = indices.len()-val.start_vertex; val});
    }

    master_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    if normal.len() > 0{master_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);}
    if uv.len() > 0{master_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);}
    if tangent.len() > 0{master_mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangent);}
    master_mesh.insert_attribute(
        MeshVertexAttribute::new("Mesh_Index", 7, VertexFormat::Uint32), 
        materials
    );
    master_mesh.set_indices(Some(Indices::U32(indices)));
    
    (master_mesh, vertex_counts)
}


pub struct CornAssetSaver;
impl AssetSaver for CornAssetSaver{
    type Asset = RawCornAsset;

    type Settings = ();

    type OutputLoader = CornAssetLoader;

    type Error = std::io::Error;

    fn save<'a>(
        &'a self,
        writer: &'a mut bevy::asset::io::Writer,
        asset: bevy::asset::saver::SavedAsset<'a, Self::Asset>,
        _settings: &'a Self::Settings,
    ) -> bevy::utils::BoxedFuture<'a, Result<<Self::OutputLoader as bevy::asset::AssetLoader>::Settings, Self::Error>> {
        Box::pin(async move {
            let mut bytes: Vec<u8> = vec![];
            /*
                Write Mesh
             */
            let topology_byte: u8 = match asset.master_mesh.primitive_topology(){
                PrimitiveTopology::LineList => 0,
                PrimitiveTopology::LineStrip => 1,
                PrimitiveTopology::PointList => 2,
                PrimitiveTopology::TriangleList => 3,
                PrimitiveTopology::TriangleStrip => 4
            };
            bytes.push(topology_byte);
            let attribute_count_byte = asset.master_mesh.attributes().count() as u8;
            bytes.push(attribute_count_byte);
            writer.write(bytes.drain(..).as_slice()).await?;
            for (id, values) in asset.master_mesh.attributes(){
                // Love that I have to do this
                let mut test_id: usize = 0;
                while MeshVertexAttributeId::from( MeshVertexAttribute::new("", test_id, VertexFormat::Float32)) != id {test_id += 1;}
                bytes.push(test_id as u8);
                let vertex_format_byte: u8 = match values {
                    VertexAttributeValues::Float32(_) => 0,
                    VertexAttributeValues::Sint32(_) => 1,
                    VertexAttributeValues::Uint32(_) => 2,
                    VertexAttributeValues::Float32x2(_) => 3,
                    VertexAttributeValues::Sint32x2(_) => 4,
                    VertexAttributeValues::Uint32x2(_) => 5,
                    VertexAttributeValues::Float32x3(_) => 6,
                    VertexAttributeValues::Sint32x3(_) => 7,
                    VertexAttributeValues::Uint32x3(_) => 8,
                    VertexAttributeValues::Float32x4(_) => 9,
                    VertexAttributeValues::Sint32x4(_) => 10,
                    VertexAttributeValues::Uint32x4(_) => 11,
                    VertexAttributeValues::Sint16x2(_) => 12,
                    VertexAttributeValues::Snorm16x2(_) => 13,
                    VertexAttributeValues::Uint16x2(_) => 14,
                    VertexAttributeValues::Unorm16x2(_) => 15,
                    VertexAttributeValues::Sint16x4(_) => 16,
                    VertexAttributeValues::Snorm16x4(_) => 17,
                    VertexAttributeValues::Uint16x4(_) => 18,
                    VertexAttributeValues::Unorm16x4(_) => 19,
                    VertexAttributeValues::Sint8x2(_) => 20,
                    VertexAttributeValues::Snorm8x2(_) => 21,
                    VertexAttributeValues::Uint8x2(_) => 22,
                    VertexAttributeValues::Unorm8x2(_) => 23,
                    VertexAttributeValues::Sint8x4(_) => 24,
                    VertexAttributeValues::Snorm8x4(_) => 25,
                    VertexAttributeValues::Uint8x4(_) => 26,
                    VertexAttributeValues::Unorm8x4(_) => 27
                };
                bytes.push(vertex_format_byte);
                let data = values.get_bytes();
                let data_length_bytes: u64 = data.len() as u64;
                bytes.extend(bytemuck::cast::<u64, [u8; 8]>(data_length_bytes));
                bytes.extend(data);
                writer.write(bytes.drain(..).as_slice()).await?;
            }
            if let Some(indices) = asset.master_mesh.indices(){
                let len = indices.len() as u64;
                bytes.extend(bytemuck::cast::<u64, [u8; 4]>(len));
                match indices{
                    Indices::U16(data) => {
                        bytes.push(0);
                        bytes.extend(data.iter().flat_map(|val| bytemuck::cast::<u16, [u8; 2]>(*val).into_iter()));
                    },
                    Indices::U32(data) => {
                        bytes.push(u8::MAX);
                        bytes.extend(data.iter().flat_map(|val| bytemuck::cast::<u32, [u8; 4]>(*val).into_iter()));
                    }
                };
            }else{
                bytes.extend(bytemuck::cast::<u64, [u8; 4]>(0));
            }
            writer.write(bytes.drain(..).as_slice()).await?;
            /*
                Write LodData
             */
            bytes.push(asset.lod_data.len() as u8);
            for lod in asset.lod_data.iter(){
                bytes.extend(bytemuck::cast::<u64, [u8; 8]>(lod.start_vertex as u64));
                bytes.extend(bytemuck::cast::<u64, [u8; 8]>(lod.total_vertices as u64));
                bytes.push(lod.sub_mesh_data.len() as u8);
                writer.write(bytes.drain(..).as_slice()).await?;
                for (vertex_count, mat_index) in lod.sub_mesh_data.iter(){
                    bytes.extend(bytemuck::cast::<u64, [u8; 8]>(*vertex_count as u64));
                    bytes.push(*mat_index as u8);
                }
                writer.write(bytes.drain(..).as_slice()).await?;
            }
            /*
                Write Standard Materials
             */
            bytes.push(asset.materials.len() as u8);
            for material in asset.materials.iter(){
                bytes.extend(material.base_color.as_rgba_u8());
                if let Some(handle) = material.base_color_texture.as_ref(){
                    bytes.push(u8::MAX);
                    match handle.id(){
                        AssetId::Uuid { uuid } => {
                            bytes.push(uuid.as_u128() as u8);
                        }
                        _ => {bytes.push(0);}
                    }
                }else{
                    bytes.push(0);
                }
                bytes.extend(material.emissive.as_rgba_u8());
                if let Some(handle) = material.emissive_texture.as_ref(){
                    bytes.push(u8::MAX);
                    match handle.id(){
                        AssetId::Uuid { uuid } => {
                            bytes.push(uuid.as_u128() as u8);
                        }
                        _ => {bytes.push(0);}
                    }
                }else{
                    bytes.push(0);
                }
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.perceptual_roughness));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.metallic));
                if let Some(handle) = material.metallic_roughness_texture.as_ref(){
                    bytes.push(u8::MAX);
                    match handle.id(){
                        AssetId::Uuid { uuid } => {
                            bytes.push(uuid.as_u128() as u8);
                        }
                        _ => {bytes.push(0);}
                    }
                }else{
                    bytes.push(0);
                }
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.reflectance));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.diffuse_transmission));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.specular_transmission));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.thickness));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.ior));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.attenuation_distance));
                bytes.extend(material.attenuation_color.as_rgba_u8());
                if let Some(handle) = material.normal_map_texture.as_ref(){
                    bytes.push(u8::MAX);
                    match handle.id(){
                        AssetId::Uuid { uuid } => {
                            bytes.push(uuid.as_u128() as u8);
                        }
                        _ => {bytes.push(0);}
                    }
                }else{
                    bytes.push(0);
                }
                if material.flip_normal_map_y{
                    bytes.push(u8::MAX);
                }else{
                    bytes.push(0);
                }
                if let Some(handle) = material.occlusion_texture.as_ref(){
                    bytes.push(u8::MAX);
                    match handle.id(){
                        AssetId::Uuid { uuid } => {
                            bytes.push(uuid.as_u128() as u8);
                        }
                        _ => {bytes.push(0);}
                    }
                }else{
                    bytes.push(0);
                }
                if material.double_sided{
                    bytes.push(u8::MAX);
                }else{
                    bytes.push(0);
                }
                if let Some(cull_mode) = material.cull_mode{
                    bytes.push(u8::MAX);
                    match cull_mode{
                        Face::Front => bytes.push(0),
                        Face::Back => bytes.push(1)
                    }
                }else{
                    bytes.push(0);
                }
                if material.unlit{
                    bytes.push(u8::MAX);
                }else{
                    bytes.push(0);
                }
                if material.fog_enabled{
                    bytes.push(u8::MAX);
                }else{
                    bytes.push(0);
                }
                match material.alpha_mode{
                    AlphaMode::Add => bytes.push(0),
                    AlphaMode::Blend => bytes.push(1),
                    AlphaMode::Multiply => bytes.push(2),
                    AlphaMode::Opaque => bytes.push(3),
                    AlphaMode::Premultiplied => bytes.push(4),
                    AlphaMode::Mask(mask) => {bytes.push(5); bytes.extend(bytemuck::cast::<f32, [u8; 4]>(mask));}
                }
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.depth_bias));
                if let Some(handle) = material.depth_map.as_ref(){
                    bytes.push(u8::MAX);
                    match handle.id(){
                        AssetId::Uuid { uuid } => {
                            bytes.push(uuid.as_u128() as u8);
                        }
                        _ => {bytes.push(0);}
                    }
                }else{
                    bytes.push(0);
                }
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.parallax_depth_scale));
                match material.parallax_mapping_method{
                    ParallaxMappingMethod::Occlusion => bytes.push(0),
                    ParallaxMappingMethod::Relief { max_steps } => {bytes.push(1); bytes.extend(bytemuck::cast::<u32, [u8; 4]>(max_steps));}
                }
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.max_parallax_layer_count));
                bytes.extend(bytemuck::cast::<f32, [u8; 4]>(material.lightmap_exposure));
                match material.opaque_render_method{
                    OpaqueRendererMethod::Auto => bytes.push(0),
                    OpaqueRendererMethod::Deferred => bytes.push(1),
                    OpaqueRendererMethod::Forward => bytes.push(2)
                }
                bytes.push(material.deferred_lighting_pass_id);
                writer.write(bytes.drain(..).as_slice()).await?;
            }
            /*
                Write Images
             */
            bytes.push(asset.textures.len() as u8);
            for texture in asset.textures.iter(){
                // data
                bytes.extend(bytemuck::cast::<u64, [u8; 8]>(texture.data.len() as u64));
                bytes.extend(texture.data.as_slice());
                // texture descriptor
                if let Some(label) = texture.texture_descriptor.label{
                    let data = label.as_bytes();
                    bytes.extend(bytemuck::cast::<u64, [u8; 8]>(data.len() as u64));
                    bytes.extend(data);
                }
                bytes.extend(bytemuck::cast::<u32, [u8; 4]>(texture.texture_descriptor.size.width));
                bytes.extend(bytemuck::cast::<u32, [u8; 4]>(texture.texture_descriptor.size.height));
                bytes.extend(bytemuck::cast::<u32, [u8; 4]>(texture.texture_descriptor.size.depth_or_array_layers));
                bytes.extend(bytemuck::cast::<u32, [u8; 4]>(texture.texture_descriptor.mip_level_count));
                bytes.extend(bytemuck::cast::<u32, [u8; 4]>(texture.texture_descriptor.sample_count));
                match texture.texture_descriptor.dimension{
                    wgpu::TextureDimension::D1 => bytes.push(0),
                    wgpu::TextureDimension::D2 => bytes.push(1),
                    wgpu::TextureDimension::D3 => bytes.push(2)
                }
                match texture.texture_descriptor.format{
                    wgpu::TextureFormat::R8Unorm => bytes.push(0),
                    wgpu::TextureFormat::R8Snorm => bytes.push(1),
                    wgpu::TextureFormat::R8Uint => bytes.push(2),
                    wgpu::TextureFormat::R8Sint => bytes.push(3),
                    wgpu::TextureFormat::R16Uint => bytes.push(4),
                    wgpu::TextureFormat::R16Sint => bytes.push(5),
                    wgpu::TextureFormat::R16Unorm => bytes.push(6),
                    wgpu::TextureFormat::R16Snorm => bytes.push(7),
                    wgpu::TextureFormat::R16Float => bytes.push(8),
                    wgpu::TextureFormat::Rg8Unorm => bytes.push(9),
                    wgpu::TextureFormat::Rg8Snorm => bytes.push(10),
                    wgpu::TextureFormat::Rg8Uint => bytes.push(11),
                    wgpu::TextureFormat::Rg8Sint => bytes.push(12),
                    wgpu::TextureFormat::R32Uint => bytes.push(13),
                    wgpu::TextureFormat::R32Sint => bytes.push(14),
                    wgpu::TextureFormat::R32Float => bytes.push(15),
                    wgpu::TextureFormat::Rg16Uint => bytes.push(16),
                    wgpu::TextureFormat::Rg16Sint => bytes.push(17),
                    wgpu::TextureFormat::Rg16Unorm => bytes.push(18),
                    wgpu::TextureFormat::Rg16Snorm => bytes.push(19),
                    wgpu::TextureFormat::Rg16Float => bytes.push(20),
                    wgpu::TextureFormat::Rgba8Unorm => bytes.push(21),
                    wgpu::TextureFormat::Rgba8UnormSrgb => bytes.push(22),
                    wgpu::TextureFormat::Rgba8Snorm => bytes.push(23),
                    wgpu::TextureFormat::Rgba8Uint => bytes.push(24),
                    wgpu::TextureFormat::Rgba8Sint => bytes.push(25),
                    wgpu::TextureFormat::Bgra8Unorm => bytes.push(26),
                    wgpu::TextureFormat::Bgra8UnormSrgb => bytes.push(27),
                    wgpu::TextureFormat::Rgb9e5Ufloat => bytes.push(28),
                    wgpu::TextureFormat::Rgb10a2Uint => bytes.push(29),
                    wgpu::TextureFormat::Rgb10a2Unorm => bytes.push(30),
                    wgpu::TextureFormat::Rg11b10Float => bytes.push(31),
                    wgpu::TextureFormat::Rg32Uint => bytes.push(32),
                    wgpu::TextureFormat::Rg32Sint => bytes.push(33),
                    wgpu::TextureFormat::Rg32Float => bytes.push(34),
                    wgpu::TextureFormat::Rgba16Uint => bytes.push(35),
                    wgpu::TextureFormat::Rgba16Sint => bytes.push(36),
                    wgpu::TextureFormat::Rgba16Unorm => bytes.push(37),
                    wgpu::TextureFormat::Rgba16Snorm => bytes.push(38),
                    wgpu::TextureFormat::Rgba16Float => bytes.push(39),
                    wgpu::TextureFormat::Rgba32Uint => bytes.push(40),
                    wgpu::TextureFormat::Rgba32Sint => bytes.push(41),
                    wgpu::TextureFormat::Rgba32Float => bytes.push(42),
                    wgpu::TextureFormat::Stencil8 => bytes.push(43),
                    wgpu::TextureFormat::Depth16Unorm => bytes.push(44),
                    wgpu::TextureFormat::Depth24Plus => bytes.push(45),
                    wgpu::TextureFormat::Depth24PlusStencil8 => bytes.push(46),
                    wgpu::TextureFormat::Depth32Float => bytes.push(47),
                    wgpu::TextureFormat::Depth32FloatStencil8 => bytes.push(48),
                    wgpu::TextureFormat::NV12 => bytes.push(49),
                    wgpu::TextureFormat::Bc1RgbaUnorm => bytes.push(50),
                    wgpu::TextureFormat::Bc1RgbaUnormSrgb => bytes.push(51),
                    wgpu::TextureFormat::Bc2RgbaUnorm => bytes.push(52),
                    wgpu::TextureFormat::Bc2RgbaUnormSrgb => bytes.push(53),
                    wgpu::TextureFormat::Bc3RgbaUnorm => bytes.push(54),
                    wgpu::TextureFormat::Bc3RgbaUnormSrgb => bytes.push(55),
                    wgpu::TextureFormat::Bc4RUnorm => bytes.push(56),
                    wgpu::TextureFormat::Bc4RSnorm => bytes.push(57),
                    wgpu::TextureFormat::Bc5RgUnorm => bytes.push(58),
                    wgpu::TextureFormat::Bc5RgSnorm => bytes.push(59),
                    wgpu::TextureFormat::Bc6hRgbUfloat => bytes.push(60),
                    wgpu::TextureFormat::Bc6hRgbFloat => bytes.push(61),
                    wgpu::TextureFormat::Bc7RgbaUnorm => bytes.push(62),
                    wgpu::TextureFormat::Bc7RgbaUnormSrgb => bytes.push(63),
                    wgpu::TextureFormat::Etc2Rgb8Unorm => bytes.push(64),
                    wgpu::TextureFormat::Etc2Rgb8UnormSrgb => bytes.push(65),
                    wgpu::TextureFormat::Etc2Rgb8A1Unorm => bytes.push(66),
                    wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb => bytes.push(67),
                    wgpu::TextureFormat::Etc2Rgba8Unorm => bytes.push(68),
                    wgpu::TextureFormat::Etc2Rgba8UnormSrgb => bytes.push(69),
                    wgpu::TextureFormat::EacR11Unorm => bytes.push(70),
                    wgpu::TextureFormat::EacR11Snorm => bytes.push(71),
                    wgpu::TextureFormat::EacRg11Unorm => bytes.push(72),
                    wgpu::TextureFormat::EacRg11Snorm => bytes.push(73),
                    wgpu::TextureFormat::Astc { block: _, channel: _ } => bytes.push(74)
                }
                bytes.extend(bytemuck::cast::<u32, [u8; 4]>(texture.texture_descriptor.usage.bits()));
                match texture.texture_descriptor.view_formats[0]{
                    wgpu::TextureFormat::R8Unorm => bytes.push(0),
                    wgpu::TextureFormat::R8Snorm => bytes.push(1),
                    wgpu::TextureFormat::R8Uint => bytes.push(2),
                    wgpu::TextureFormat::R8Sint => bytes.push(3),
                    wgpu::TextureFormat::R16Uint => bytes.push(4),
                    wgpu::TextureFormat::R16Sint => bytes.push(5),
                    wgpu::TextureFormat::R16Unorm => bytes.push(6),
                    wgpu::TextureFormat::R16Snorm => bytes.push(7),
                    wgpu::TextureFormat::R16Float => bytes.push(8),
                    wgpu::TextureFormat::Rg8Unorm => bytes.push(9),
                    wgpu::TextureFormat::Rg8Snorm => bytes.push(10),
                    wgpu::TextureFormat::Rg8Uint => bytes.push(11),
                    wgpu::TextureFormat::Rg8Sint => bytes.push(12),
                    wgpu::TextureFormat::R32Uint => bytes.push(13),
                    wgpu::TextureFormat::R32Sint => bytes.push(14),
                    wgpu::TextureFormat::R32Float => bytes.push(15),
                    wgpu::TextureFormat::Rg16Uint => bytes.push(16),
                    wgpu::TextureFormat::Rg16Sint => bytes.push(17),
                    wgpu::TextureFormat::Rg16Unorm => bytes.push(18),
                    wgpu::TextureFormat::Rg16Snorm => bytes.push(19),
                    wgpu::TextureFormat::Rg16Float => bytes.push(20),
                    wgpu::TextureFormat::Rgba8Unorm => bytes.push(21),
                    wgpu::TextureFormat::Rgba8UnormSrgb => bytes.push(22),
                    wgpu::TextureFormat::Rgba8Snorm => bytes.push(23),
                    wgpu::TextureFormat::Rgba8Uint => bytes.push(24),
                    wgpu::TextureFormat::Rgba8Sint => bytes.push(25),
                    wgpu::TextureFormat::Bgra8Unorm => bytes.push(26),
                    wgpu::TextureFormat::Bgra8UnormSrgb => bytes.push(27),
                    wgpu::TextureFormat::Rgb9e5Ufloat => bytes.push(28),
                    wgpu::TextureFormat::Rgb10a2Uint => bytes.push(29),
                    wgpu::TextureFormat::Rgb10a2Unorm => bytes.push(30),
                    wgpu::TextureFormat::Rg11b10Float => bytes.push(31),
                    wgpu::TextureFormat::Rg32Uint => bytes.push(32),
                    wgpu::TextureFormat::Rg32Sint => bytes.push(33),
                    wgpu::TextureFormat::Rg32Float => bytes.push(34),
                    wgpu::TextureFormat::Rgba16Uint => bytes.push(35),
                    wgpu::TextureFormat::Rgba16Sint => bytes.push(36),
                    wgpu::TextureFormat::Rgba16Unorm => bytes.push(37),
                    wgpu::TextureFormat::Rgba16Snorm => bytes.push(38),
                    wgpu::TextureFormat::Rgba16Float => bytes.push(39),
                    wgpu::TextureFormat::Rgba32Uint => bytes.push(40),
                    wgpu::TextureFormat::Rgba32Sint => bytes.push(41),
                    wgpu::TextureFormat::Rgba32Float => bytes.push(42),
                    wgpu::TextureFormat::Stencil8 => bytes.push(43),
                    wgpu::TextureFormat::Depth16Unorm => bytes.push(44),
                    wgpu::TextureFormat::Depth24Plus => bytes.push(45),
                    wgpu::TextureFormat::Depth24PlusStencil8 => bytes.push(46),
                    wgpu::TextureFormat::Depth32Float => bytes.push(47),
                    wgpu::TextureFormat::Depth32FloatStencil8 => bytes.push(48),
                    wgpu::TextureFormat::NV12 => bytes.push(49),
                    wgpu::TextureFormat::Bc1RgbaUnorm => bytes.push(50),
                    wgpu::TextureFormat::Bc1RgbaUnormSrgb => bytes.push(51),
                    wgpu::TextureFormat::Bc2RgbaUnorm => bytes.push(52),
                    wgpu::TextureFormat::Bc2RgbaUnormSrgb => bytes.push(53),
                    wgpu::TextureFormat::Bc3RgbaUnorm => bytes.push(54),
                    wgpu::TextureFormat::Bc3RgbaUnormSrgb => bytes.push(55),
                    wgpu::TextureFormat::Bc4RUnorm => bytes.push(56),
                    wgpu::TextureFormat::Bc4RSnorm => bytes.push(57),
                    wgpu::TextureFormat::Bc5RgUnorm => bytes.push(58),
                    wgpu::TextureFormat::Bc5RgSnorm => bytes.push(59),
                    wgpu::TextureFormat::Bc6hRgbUfloat => bytes.push(60),
                    wgpu::TextureFormat::Bc6hRgbFloat => bytes.push(61),
                    wgpu::TextureFormat::Bc7RgbaUnorm => bytes.push(62),
                    wgpu::TextureFormat::Bc7RgbaUnormSrgb => bytes.push(63),
                    wgpu::TextureFormat::Etc2Rgb8Unorm => bytes.push(64),
                    wgpu::TextureFormat::Etc2Rgb8UnormSrgb => bytes.push(65),
                    wgpu::TextureFormat::Etc2Rgb8A1Unorm => bytes.push(66),
                    wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb => bytes.push(67),
                    wgpu::TextureFormat::Etc2Rgba8Unorm => bytes.push(68),
                    wgpu::TextureFormat::Etc2Rgba8UnormSrgb => bytes.push(69),
                    wgpu::TextureFormat::EacR11Unorm => bytes.push(70),
                    wgpu::TextureFormat::EacR11Snorm => bytes.push(71),
                    wgpu::TextureFormat::EacRg11Unorm => bytes.push(72),
                    wgpu::TextureFormat::EacRg11Snorm => bytes.push(73),
                    wgpu::TextureFormat::Astc { block: _, channel: _ } => bytes.push(74)
                }
                if let ImageSampler::Descriptor(desc) = &texture.sampler{
                    bytes.push(u8::MAX);
                    if let Some(label) = desc.label.as_ref(){
                        bytes.push(u8::MAX);
                        let data = label.as_str().as_bytes();
                        bytes.extend(bytemuck::cast::<u64, [u8; 8]>(data.len() as u64));
                        bytes.extend(data);
                    }else{
                        bytes.push(0);
                    }
                    match desc.address_mode_u{
                        ImageAddressMode::ClampToBorder => bytes.push(0),
                        ImageAddressMode::ClampToEdge => bytes.push(1),
                        ImageAddressMode::MirrorRepeat => bytes.push(2),
                        ImageAddressMode::Repeat => bytes.push(3)
                    }
                    match desc.address_mode_v{
                        ImageAddressMode::ClampToBorder => bytes.push(0),
                        ImageAddressMode::ClampToEdge => bytes.push(1),
                        ImageAddressMode::MirrorRepeat => bytes.push(2),
                        ImageAddressMode::Repeat => bytes.push(3)
                    }
                    match desc.address_mode_w{
                        ImageAddressMode::ClampToBorder => bytes.push(0),
                        ImageAddressMode::ClampToEdge => bytes.push(1),
                        ImageAddressMode::MirrorRepeat => bytes.push(2),
                        ImageAddressMode::Repeat => bytes.push(3)
                    }
                    match desc.mipmap_filter{
                        ImageFilterMode::Linear => bytes.push(0),
                        ImageFilterMode::Nearest => bytes.push(1),
                    }
                    match desc.min_filter{
                        ImageFilterMode::Linear => bytes.push(0),
                        ImageFilterMode::Nearest => bytes.push(1),
                    }
                    match desc.mag_filter{
                        ImageFilterMode::Linear => bytes.push(0),
                        ImageFilterMode::Nearest => bytes.push(1),
                    }
                    bytes.extend(bytemuck::cast::<f32, [u8; 4]>(desc.lod_min_clamp));
                    bytes.extend(bytemuck::cast::<f32, [u8; 4]>(desc.lod_max_clamp));
                    if let Some(comp) = desc.compare{
                        bytes.push(u8::MAX);
                        match comp {
                            bevy::render::texture::ImageCompareFunction::Never => bytes.push(0),
                            bevy::render::texture::ImageCompareFunction::Less => bytes.push(1),
                            bevy::render::texture::ImageCompareFunction::Equal => bytes.push(2),
                            bevy::render::texture::ImageCompareFunction::LessEqual => bytes.push(3),
                            bevy::render::texture::ImageCompareFunction::Greater => bytes.push(4),
                            bevy::render::texture::ImageCompareFunction::NotEqual => bytes.push(5),
                            bevy::render::texture::ImageCompareFunction::GreaterEqual => bytes.push(6),
                            bevy::render::texture::ImageCompareFunction::Always => bytes.push(7),
                        }
                    }else{
                        bytes.push(0);
                    }
                    bytes.extend(bytemuck::cast::<u16, [u8; 2]>(desc.anisotropy_clamp));
                    if let Some(col) = desc.border_color{
                        bytes.push(u8::MAX);
                        match col {
                            bevy::render::texture::ImageSamplerBorderColor::TransparentBlack => bytes.push(0),
                            bevy::render::texture::ImageSamplerBorderColor::OpaqueBlack => bytes.push(1),
                            bevy::render::texture::ImageSamplerBorderColor::OpaqueWhite => bytes.push(2),
                            bevy::render::texture::ImageSamplerBorderColor::Zero => bytes.push(3),
                        }
                    }else{
                        bytes.push(0);
                    }

                }else{
                    bytes.push(0);
                }
                if let Some(desc) = &texture.texture_view_descriptor{
                    bytes.push(u8::MAX);
                    if let Some(label) = desc.label{
                        bytes.push(u8::MAX);
                        let data = label.as_bytes();
                        bytes.extend(bytemuck::cast::<u64, [u8; 8]>(data.len() as u64));
                        bytes.extend(data);
                    }else{
                        bytes.push(0);
                    }
                    if let Some(format) = desc.format{
                        bytes.push(u8::MAX);
                        match format{
                            wgpu::TextureFormat::R8Unorm => bytes.push(0),
                            wgpu::TextureFormat::R8Snorm => bytes.push(1),
                            wgpu::TextureFormat::R8Uint => bytes.push(2),
                            wgpu::TextureFormat::R8Sint => bytes.push(3),
                            wgpu::TextureFormat::R16Uint => bytes.push(4),
                            wgpu::TextureFormat::R16Sint => bytes.push(5),
                            wgpu::TextureFormat::R16Unorm => bytes.push(6),
                            wgpu::TextureFormat::R16Snorm => bytes.push(7),
                            wgpu::TextureFormat::R16Float => bytes.push(8),
                            wgpu::TextureFormat::Rg8Unorm => bytes.push(9),
                            wgpu::TextureFormat::Rg8Snorm => bytes.push(10),
                            wgpu::TextureFormat::Rg8Uint => bytes.push(11),
                            wgpu::TextureFormat::Rg8Sint => bytes.push(12),
                            wgpu::TextureFormat::R32Uint => bytes.push(13),
                            wgpu::TextureFormat::R32Sint => bytes.push(14),
                            wgpu::TextureFormat::R32Float => bytes.push(15),
                            wgpu::TextureFormat::Rg16Uint => bytes.push(16),
                            wgpu::TextureFormat::Rg16Sint => bytes.push(17),
                            wgpu::TextureFormat::Rg16Unorm => bytes.push(18),
                            wgpu::TextureFormat::Rg16Snorm => bytes.push(19),
                            wgpu::TextureFormat::Rg16Float => bytes.push(20),
                            wgpu::TextureFormat::Rgba8Unorm => bytes.push(21),
                            wgpu::TextureFormat::Rgba8UnormSrgb => bytes.push(22),
                            wgpu::TextureFormat::Rgba8Snorm => bytes.push(23),
                            wgpu::TextureFormat::Rgba8Uint => bytes.push(24),
                            wgpu::TextureFormat::Rgba8Sint => bytes.push(25),
                            wgpu::TextureFormat::Bgra8Unorm => bytes.push(26),
                            wgpu::TextureFormat::Bgra8UnormSrgb => bytes.push(27),
                            wgpu::TextureFormat::Rgb9e5Ufloat => bytes.push(28),
                            wgpu::TextureFormat::Rgb10a2Uint => bytes.push(29),
                            wgpu::TextureFormat::Rgb10a2Unorm => bytes.push(30),
                            wgpu::TextureFormat::Rg11b10Float => bytes.push(31),
                            wgpu::TextureFormat::Rg32Uint => bytes.push(32),
                            wgpu::TextureFormat::Rg32Sint => bytes.push(33),
                            wgpu::TextureFormat::Rg32Float => bytes.push(34),
                            wgpu::TextureFormat::Rgba16Uint => bytes.push(35),
                            wgpu::TextureFormat::Rgba16Sint => bytes.push(36),
                            wgpu::TextureFormat::Rgba16Unorm => bytes.push(37),
                            wgpu::TextureFormat::Rgba16Snorm => bytes.push(38),
                            wgpu::TextureFormat::Rgba16Float => bytes.push(39),
                            wgpu::TextureFormat::Rgba32Uint => bytes.push(40),
                            wgpu::TextureFormat::Rgba32Sint => bytes.push(41),
                            wgpu::TextureFormat::Rgba32Float => bytes.push(42),
                            wgpu::TextureFormat::Stencil8 => bytes.push(43),
                            wgpu::TextureFormat::Depth16Unorm => bytes.push(44),
                            wgpu::TextureFormat::Depth24Plus => bytes.push(45),
                            wgpu::TextureFormat::Depth24PlusStencil8 => bytes.push(46),
                            wgpu::TextureFormat::Depth32Float => bytes.push(47),
                            wgpu::TextureFormat::Depth32FloatStencil8 => bytes.push(48),
                            wgpu::TextureFormat::NV12 => bytes.push(49),
                            wgpu::TextureFormat::Bc1RgbaUnorm => bytes.push(50),
                            wgpu::TextureFormat::Bc1RgbaUnormSrgb => bytes.push(51),
                            wgpu::TextureFormat::Bc2RgbaUnorm => bytes.push(52),
                            wgpu::TextureFormat::Bc2RgbaUnormSrgb => bytes.push(53),
                            wgpu::TextureFormat::Bc3RgbaUnorm => bytes.push(54),
                            wgpu::TextureFormat::Bc3RgbaUnormSrgb => bytes.push(55),
                            wgpu::TextureFormat::Bc4RUnorm => bytes.push(56),
                            wgpu::TextureFormat::Bc4RSnorm => bytes.push(57),
                            wgpu::TextureFormat::Bc5RgUnorm => bytes.push(58),
                            wgpu::TextureFormat::Bc5RgSnorm => bytes.push(59),
                            wgpu::TextureFormat::Bc6hRgbUfloat => bytes.push(60),
                            wgpu::TextureFormat::Bc6hRgbFloat => bytes.push(61),
                            wgpu::TextureFormat::Bc7RgbaUnorm => bytes.push(62),
                            wgpu::TextureFormat::Bc7RgbaUnormSrgb => bytes.push(63),
                            wgpu::TextureFormat::Etc2Rgb8Unorm => bytes.push(64),
                            wgpu::TextureFormat::Etc2Rgb8UnormSrgb => bytes.push(65),
                            wgpu::TextureFormat::Etc2Rgb8A1Unorm => bytes.push(66),
                            wgpu::TextureFormat::Etc2Rgb8A1UnormSrgb => bytes.push(67),
                            wgpu::TextureFormat::Etc2Rgba8Unorm => bytes.push(68),
                            wgpu::TextureFormat::Etc2Rgba8UnormSrgb => bytes.push(69),
                            wgpu::TextureFormat::EacR11Unorm => bytes.push(70),
                            wgpu::TextureFormat::EacR11Snorm => bytes.push(71),
                            wgpu::TextureFormat::EacRg11Unorm => bytes.push(72),
                            wgpu::TextureFormat::EacRg11Snorm => bytes.push(73),
                            wgpu::TextureFormat::Astc { block: _, channel: _ } => bytes.push(74)
                        }
                    }else{
                        bytes.push(0);
                    }
                    if let Some(dimension) = desc.dimension{
                        bytes.push(u8::MAX);
                        match dimension{
                            TextureViewDimension::D1 => bytes.push(0),
                            TextureViewDimension::D2 => bytes.push(1),
                            TextureViewDimension::D2Array => bytes.push(2),
                            TextureViewDimension::Cube => bytes.push(3),
                            TextureViewDimension::CubeArray => bytes.push(4),
                            TextureViewDimension::D3 => bytes.push(5),
                        }
                        
                    }else{
                        bytes.push(0);
                    }
                    match desc.aspect{
                        wgpu::TextureAspect::All => bytes.push(0),
                        wgpu::TextureAspect::StencilOnly => bytes.push(1),
                        wgpu::TextureAspect::DepthOnly => bytes.push(2),
                        wgpu::TextureAspect::Plane0 => bytes.push(3),
                        wgpu::TextureAspect::Plane1 => bytes.push(4),
                        wgpu::TextureAspect::Plane2 => bytes.push(5)
                    }
                    bytes.extend(bytemuck::cast::<u32, [u8; 4]>(desc.base_mip_level));
                    if let Some(mip) = desc.mip_level_count{
                        bytes.push(u8::MAX);
                        bytes.extend(bytemuck::cast::<u32, [u8; 4]>(mip));
                    }else{
                        bytes.push(0);
                    }
                    bytes.extend(bytemuck::cast::<u32, [u8; 4]>(desc.base_array_layer));
                    if let Some(array) = desc.array_layer_count{
                        bytes.push(u8::MAX);
                        bytes.extend(bytemuck::cast::<u32, [u8; 4]>(array));
                    }else{
                        bytes.push(0);
                    }
                }else{
                    bytes.push(0);
                }
                if texture.asset_usage.contains(RenderAssetUsages::MAIN_WORLD) {
                    bytes.push(1);
                }else{
                    bytes.push(0);
                }
                if texture.asset_usage.contains(RenderAssetUsages::RENDER_WORLD) {
                    bytes.push(1);
                }else{
                    bytes.push(0);
                }
                writer.write(bytes.drain(..).as_slice()).await?;
            }
            writer.write(bytes.drain(..).as_slice()).await?;
            return Ok(());
        })
    }
}


pub struct CornAssetLoader;
impl AssetLoader for CornAssetLoader{
    type Asset = RawCornAsset;

    type Settings = ();

    type Error = Infallible;

    fn load<'a>(
        &'a self,
        _reader: &'a mut bevy::asset::io::Reader,
        _settings: &'a Self::Settings,
        _load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            return Ok(RawCornAsset{master_mesh: Mesh::new(wgpu::PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD), lod_data: vec![], materials: vec![], textures: vec![]});
        })
    }

    fn extensions(&self) -> &[&str] {
        &["corn"]
    }
}
