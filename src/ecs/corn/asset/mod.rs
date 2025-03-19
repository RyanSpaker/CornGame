pub mod processing;
//pub mod voxel_auto_lod;

use bevy::{
    asset::processor::LoadTransformAndSave, 
    gltf::GltfLoader, 
    prelude::*, 
    render::{render_asset::{RenderAsset, RenderAssetPlugin, RenderAssetUsages}, Extract, RenderApp}, 
    utils::hashbrown::HashMap
};
use crate::util::asset_io::{read_each, read_u64, write_byte, write_each, write_u64, read_byte};
use self::processing::*;

/// Adds functionality to the app to Process and Load the Corn Model into the app
pub struct CornAssetPlugin;
impl Plugin for CornAssetPlugin{
    fn build(&self, app: &mut App) {
        app
            //.add_plugins(auto_lod::AutoLodPlugin)
            .register_asset_processor::<LoadTransformAndSave<GltfLoader, CornAssetTransformer, CornAssetSaver>>(LoadTransformAndSave::new(CornAssetTransformer, CornAssetSaver))
            .register_asset_loader::<CornAssetLoader>(CornAssetLoader)
            .init_asset::<CornAsset>()
            .add_plugins(RenderAssetPlugin::<CornAsset>::default())
            .init_resource::<CornModel>()
            .register_type::<CornModel>()
            .register_type::<CornAsset>()
            .register_type::<CornMeshLod>()
            .add_systems(Startup, spawn_corn_asset)
            .add_systems(Update, CornModel::enable_asset.run_if(on_event::<AssetEvent<CornAsset>>))
            .sub_app_mut(RenderApp)
                .init_resource::<CornModel>()
                .add_systems(ExtractSchedule, CornModel::clone_corn_resource);
    }
}
/// Loads the corn model from the assets folder. Runs when the app starts
pub fn spawn_corn_asset(mut corn_res: ResMut<CornModel>, assets: Res<AssetServer>){
    corn_res.asset = assets.load("models/Corn.glb");
}

/// Resource which holds some commonly accessed data about the corn Model, as well as a handle to the [`CornAsset`]
#[derive(Default, Debug, Clone, Resource, Reflect)]
pub struct CornModel{
    pub lod_count: usize,
    pub loaded: bool,
    pub asset: Handle<CornAsset>
}
impl CornModel{
    fn enable_asset(mut corn: ResMut<CornModel>, assets: Res<Assets<CornAsset>>, mut events: EventReader<AssetEvent<CornAsset>>){
        if !events.read().any(|event| event.is_loaded_with_dependencies(corn.asset.id())) {return;}
        if let Some(asset) = assets.get(&corn.asset){
            corn.loaded = true;
            corn.lod_count = asset.lod_count;
        }
    }
    fn clone_corn_resource(mut render_corn: ResMut<CornModel>, main_corn: Extract<Res<CornModel>>){
        if main_corn.loaded && (!render_corn.loaded || main_corn.is_changed()){
            *render_corn = main_corn.clone();
        }
    }
}

/// Asset which holds handles and info about the Corn Model
#[derive(Default, Debug, Reflect, Asset, Clone)]
pub struct CornAsset{
    pub master_mesh: Handle<Mesh>,
    pub lod_data: Vec<CornMeshLod>,
    pub lod_count: usize,
    pub materials: HashMap<String, Handle<StandardMaterial>>,
    pub textures: HashMap<String, Handle<Image>>
}
impl RenderAsset for CornAsset{
    type SourceAsset = Self;

    type Param = ();

    fn asset_usage(_source_asset: &Self::SourceAsset) -> RenderAssetUsages {
        RenderAssetUsages::all()
    }

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _param: &mut bevy::ecs::system::SystemParamItem<Self::Param>,
    ) -> Result<Self, bevy::render::render_asset::PrepareAssetError<Self>> {
        // NOTE: classic example where rust needs conditional default impls in traits, I should not have to write this function
        Ok(source_asset)
    }
}
/// Holds info about a specific corn lod
#[derive(Default, Debug, Clone, Reflect)]
pub struct CornMeshLod{
    /// Index of the first vertex for this lod in the master corn mesh
    pub start_vertex: usize,
    /// Total number of vertices in this lod
    pub total_vertices: usize,
    /// For each sub_mesh, (Vertex Count, Material Index)
    /// Unfortunalte material index is useless atm because materials are stored in a hashmap not a vector
    pub sub_mesh_data: Vec<(usize, usize)>,
}
impl CornMeshLod{
    pub fn from_start(start_vertex: usize) -> Self{ Self{start_vertex, total_vertices: 0, sub_mesh_data: vec![]} }
    pub async fn write(&self, writer: &mut bevy::asset::io::Writer, counter: &mut usize) -> Result<(), std::io::Error>{
        write_u64(self.start_vertex as u64, writer, counter).await?;
        write_u64(self.total_vertices as u64, writer, counter).await?;
        for (vertex_count, mat_index) in write_each(writer, counter, &self.sub_mesh_data).await?.into_iter(){
            write_u64(*vertex_count as u64, writer, counter).await?;
            write_byte(*mat_index as u8, writer, counter).await?;
        }
        Ok(())
    }
    pub async fn read<'a>(reader: &'a mut dyn bevy::asset::io::Reader, counter: &mut usize) -> Result<Self, std::io::Error>{
        let start_vertex = read_u64(reader, counter).await? as usize;
        let total_vertices = read_u64(reader, counter).await? as usize;
        let mut sub_mesh_data: Vec<(usize, usize)> = vec![];
        for _ in read_each(reader, counter).await?{
            sub_mesh_data.push((
                read_u64(reader, counter).await? as usize,
                read_byte(reader, counter).await? as usize
            ));
        }
        Ok(Self{start_vertex, total_vertices, sub_mesh_data})
    }
}
