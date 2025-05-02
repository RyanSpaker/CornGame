use std::convert::Infallible;
use bevy::{
    asset::{saver::AssetSaver, transformer::{AssetTransformer, TransformedAsset}, AssetLoader}, 
    gltf::{Gltf, GltfMesh}, 
    prelude::*, 
    render::{mesh::{Indices, MeshVertexAttribute, VertexAttributeValues}, render_asset::RenderAssetUsages}, 
    utils::{hashbrown::HashMap}
};
use uuid::Uuid;
use wgpu_types::VertexFormat;
use super::{CornMeshLod, CornAsset};
use crate::util::asset_io::{*, mesh_io::{save_mesh, read_mesh}, image_io::{save_image, read_image}, standard_material_io::{read_standard_material, save_standard_material}};

/// Asset which holds the Corn model data to be saved to disk after being extracted from a [`Gltf`]
#[derive(Debug, Clone, TypePath, Asset)]
pub struct RawCornAsset{
    master_mesh: Mesh,
    lod_data: Vec<CornMeshLod>,
    materials: Vec<(StandardMaterial, String)>,
    textures: Vec<(Image, String)>
}

/// Takes a [`Gltf`] Asset, extracts the corn model, and turns it into a [`RawCornAsset`] to be saved
#[derive(Debug, Reflect)]
pub struct CornAssetTransformer;
impl CornAssetTransformer{
    /// Given a vector of lods, each a vector of Mesh Label, material index, Combine the meshes into a single master mesh, and track vertex information, returning both.
    async fn combine_corn_mesh(lods: Vec<Vec<(String, usize)>>, asset: &mut TransformedAsset<Gltf>) -> (Mesh, Vec<CornMeshLod>) {
        let mut master_mesh = Mesh::new(wgpu::PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        let mut vertex_counts: Vec<CornMeshLod> = vec![];
        // Setup Attribute Lists
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normal: Vec<[f32; 3]> = Vec::new();
        let mut uv: Vec<[f32; 2]> = Vec::new();
        let mut tangent: Vec<[f32; 4]> = Vec::new();
        let mut materials: Vec<[u16; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        let mut indices_offset: u32 = 0;
        for lod in lods.iter(){
            vertex_counts.push(CornMeshLod::from_start(indices.len()));
            for (mesh_label, mat) in lod.iter(){
                let transformed = asset.get_labeled::<Mesh, str>(mesh_label.as_str()).unwrap();
                let mesh = transformed.get();
                match mesh.indices(){
                    Some(Indices::U16(values)) => {
                        indices.extend(values.iter().map(|ind| *ind as u32 + indices_offset));
                        vertex_counts.last_mut().unwrap().sub_mesh_data.push((values.len(), *mat));
                    },
                    Some(Indices::U32(values)) => {
                        indices.extend(values.iter().map(|ind| ind + indices_offset));
                        vertex_counts.last_mut().unwrap().sub_mesh_data.push((values.len(), *mat));
                    },
                    None => {panic!("Corn Mesh had no Indices {:?}", mesh);}
                }
                match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                    Some(VertexAttributeValues::Float32x3(values)) => {
                        positions.extend(values); 
                        materials.extend([[*mat as u16, 0u16]].repeat(values.len()));
                        indices_offset += values.len() as u32;
                    },
                    _ => {panic!("Corn Mesh had no Position Attribute");}
                }
                if let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL){
                    normal.extend(normals);
                }
                if let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0){
                    uv.extend(uvs);
                }
                if let Some(VertexAttributeValues::Float32x4(tangents)) = mesh.attribute(Mesh::ATTRIBUTE_TANGENT){
                    tangent.extend(tangents);
                }
            }
            vertex_counts.last_mut().map(|val| {val.total_vertices = indices.len()-val.start_vertex; val});
        }
        let indices = if indices.iter().all(|index| *index <= u16::MAX as u32) {
            Indices::U16(indices.into_iter().map(|i| i as u16).collect())
        } else {Indices::U32(indices)};
        master_mesh.insert_indices(indices);
        master_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        if normal.len() > 0{master_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal);}
        if uv.len() > 0{master_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);}
        if tangent.len() > 0{master_mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangent);}
        master_mesh.insert_attribute(
            MeshVertexAttribute::new("Mesh_Material_Index", 23895, VertexFormat::Uint16x2), 
            VertexAttributeValues::Uint16x2(materials)
        );
        (master_mesh, vertex_counts)
    }
}
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
                named_meshes.insert(handle.to_owned(), (gltf_mesh_labels.get(handle).unwrap().to_owned(), name.to_string()));
            }
            // List of Materials used by the corn mesh
            let mut used_materials: Vec<Handle<StandardMaterial>> = vec![];
            // List of Lods each a list of meshes and the index of the material that mesh uses
            let mut lods: Vec<Vec<(Handle<Mesh>, usize)>> = vec![];
            for (_, (label,name)) in named_meshes.iter(){
                dbg!(&name);
                let Some((_, tail)) = name.split_once("CornLOD") else {continue}; //XXX fixme hardcoded
                let (lod_level, mesh_index) = tail.split_once(".").unwrap_or((tail, "0"));

                let sub_asset = asset.get_labeled::<GltfMesh, str>(label).unwrap();
                let gltf_mesh = sub_asset.get();
                let primitive = &gltf_mesh.primitives[0];
                let mat =  primitive.material.as_ref().unwrap().to_owned();
                let mesh = primitive.mesh.to_owned();
                let pos = if let Some(pos) = used_materials.iter().position(|e| *e == mat) {pos} else {
                    used_materials.push(mat.to_owned());
                    used_materials.len() - 1
                };
                let lod_level = lod_level.parse::<usize>().unwrap(); 
                let mesh_index = mesh_index.parse::<usize>().unwrap();
                while lods.len() <= lod_level {lods.push(vec![]);}
                while lods[lod_level].len() <= mesh_index {lods[lod_level].push((Handle::default(), 0));}
                lods[lod_level][mesh_index] = (mesh, pos);
            }
            // Label, mat_index
            let lods: Vec<Vec<(String, usize)>> = lods.into_iter().map(|lod| lod.into_iter().map(|(handle, mat)| 
                (mesh_labels.get(&handle).unwrap().to_owned(), mat)
            ).collect()).collect();

            let mut used_materials: Vec<(StandardMaterial, String)> = used_materials.into_iter().map(|handle| {
                let label = material_labels.get(&handle).unwrap();
                let name = asset.named_materials.iter().find(|(_, h)| **h == handle).unwrap().0.clone();
                let transformed = asset.get_labeled::<StandardMaterial, str>(label.as_str()).unwrap();
                dbg!(&name);
                (transformed.get().clone(), name.to_string())
            }).collect();

            let (master_mesh, vertex_counts) = Self::combine_corn_mesh(lods, &mut asset).await;

            let mut used_images: Vec<Handle<Image>> = vec![];
            for (mat, _) in used_materials.iter_mut(){
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
            
            let used_images: Vec<(Image, String)> = used_images.into_iter().map(|handle| {
                let transform = asset.get_labeled::<Image, str>(texture_labels.get(&handle).unwrap()).unwrap();
                let image = transform.get().clone();
                drop(transform);
                let name = texture_labels.get(&handle).unwrap().clone();
                (image, name)
            }).collect();

            return Ok(asset.replace_asset(RawCornAsset{
                master_mesh, lod_data: vertex_counts, materials: used_materials, textures: used_images
            }));
        })
    }
}

/// Saves a [`RawCornAsset`] to disk
pub struct CornAssetSaver;
impl AssetSaver for CornAssetSaver{
    type Asset = RawCornAsset;

    type Settings = ();

    type OutputLoader = CornAssetLoader;

    type Error = std::io::Error;

    fn save(
        &self,
        writer: &mut bevy::asset::io::Writer,
        asset: bevy::asset::saver::SavedAsset<'_, Self::Asset>,
        settings: &Self::Settings,
    ) -> impl bevy::utils::ConditionalSendFuture<
        Output = Result<<Self::OutputLoader as AssetLoader>::Settings, Self::Error>,
    > {
        Box::pin(async move {
            let mut byte_count: usize = 0;
            byte_count += save_mesh(&asset.master_mesh, writer).await?;
            for lod in write_each(writer, &mut byte_count, &asset.lod_data).await?.into_iter(){
                lod.write(writer, &mut byte_count).await?;
            }
            for (mat, name) in write_each(writer, &mut byte_count, &asset.materials).await?.into_iter(){
                write_string(writer, &mut byte_count, name).await?;
                byte_count += save_standard_material(mat, writer).await?;
            }
            for (tex, name) in write_each(writer, &mut byte_count, &asset.textures).await?.into_iter(){
                write_string(writer, &mut byte_count, name).await?;
                save_image(tex, writer, &mut byte_count).await?;
            }
            return Ok(());
        })        
    }
}

/// Reads a [`RawCornAsset`], and turns it into a [`CornAsset`]
pub struct CornAssetLoader;
impl AssetLoader for CornAssetLoader{
    type Asset = CornAsset;

    type Settings = ();

    type Error = std::io::Error;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::utils::ConditionalSendFuture<Output = Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut byte_counter: usize = 0;
            let mesh = read_mesh(reader, &mut byte_counter).await?;
            let sub_mesh_count = read_u64(reader, &mut byte_counter).await? as usize;
            let mut sub_mesh_data: Vec<CornMeshLod> = Vec::with_capacity(sub_mesh_count);
            for _ in 0..sub_mesh_count{
                sub_mesh_data.push(CornMeshLod::read(reader, &mut byte_counter).await?);
            }
            let mat_number = read_u64(reader, &mut byte_counter).await? as usize;
            let mut materials: Vec<(StandardMaterial, String)> = Vec::with_capacity(mat_number);
            for _ in 0..mat_number{
                let name = read_string(reader, &mut byte_counter).await?;
                let mat = read_standard_material(reader, &mut byte_counter).await?;
                materials.push((mat, name));
            }
            let tex_number = read_u64(reader, &mut byte_counter).await? as usize;
            let mut textures: Vec<(Image, String)> = Vec::with_capacity(tex_number);
            for i in 0..tex_number{
                dbg!(i, tex_number);
                let name = read_string(reader, &mut byte_counter).await?;
                dbg!(&name);
                let image = read_image(reader, &mut byte_counter).await?;
                textures.push((image, name));
            }
            /*
                Add LabeledAssets
                Assign Images to Materials
            */
            let textures: Vec<(String, Handle<Image>)> = textures.into_iter().map(|(tex, name)| {
                (name.clone(), load_context.add_labeled_asset(name, tex))
            }).collect();
            let materials: Vec<(String, Handle<StandardMaterial>)> = materials.into_iter().map(|(mut mat, name)| {
                if let Some(handle) = mat.base_color_texture{
                    let index = match handle.id() {AssetId::Uuid { uuid } => {uuid.as_u128() as usize}, _ => {0}};
                    mat.base_color_texture = Some(textures[index].1.clone());
                }
                if let Some(handle) = mat.emissive_texture{
                    let index = match handle.id() {AssetId::Uuid { uuid } => {uuid.as_u128() as usize}, _ => {0}};
                    mat.emissive_texture = Some(textures[index].1.clone());
                }
                if let Some(handle) = mat.metallic_roughness_texture{
                    let index = match handle.id() {AssetId::Uuid { uuid } => {uuid.as_u128() as usize}, _ => {0}};
                    mat.metallic_roughness_texture = Some(textures[index].1.clone());
                }
                if let Some(handle) = mat.normal_map_texture{
                    let index = match handle.id() {AssetId::Uuid { uuid } => {uuid.as_u128() as usize}, _ => {0}};
                    mat.normal_map_texture = Some(textures[index].1.clone());
                }
                if let Some(handle) = mat.occlusion_texture{
                    let index = match handle.id() {AssetId::Uuid { uuid } => {uuid.as_u128() as usize}, _ => {0}};
                    mat.occlusion_texture = Some(textures[index].1.clone());
                }
                if let Some(handle) = mat.depth_map{
                    let index = match handle.id() {AssetId::Uuid { uuid } => {uuid.as_u128() as usize}, _ => {0}};
                    mat.depth_map = Some(textures[index].1.clone());
                }
                (name.clone(), load_context.add_labeled_asset(name, mat))
            }).collect();
            let master_mesh = load_context.add_labeled_asset("Master Corn Mesh".to_string(), mesh);
            return Ok(CornAsset{
                master_mesh,
                lod_count: sub_mesh_data.len(),
                lod_data: sub_mesh_data,
                materials: HashMap::from_iter(materials.into_iter()),
                textures: HashMap::from_iter(textures.into_iter()),
            });
        })
    }

    fn extensions(&self) -> &[&str] {
        &["corn"]
    }
}
