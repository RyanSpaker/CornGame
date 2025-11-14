//! # Corn Model Loading Pipeline
//! 
//! ## Main World Corn Model Entity
//! - Corn Mesh / Lod Info: Required for all corn models. Holds the mesh handle and the lod info.
//! - CornFields: Relation Target, holds corn fields which use this corn model
//! 
//! - Corn GLTF: Produces Mesh and sender
//! - Corn Load Sender: Sends data to mesh and creates lod info
//! - Corn Modify Task: Recreates lod info and mesh
//! 
//! ## Render World Corn Model Entity
//! - Corn Mesh
//! - Corn Lod Info: Produces Indirect Buffer
//! - Indirect Buffer
//! 
//! ## Main World Corn Field Entity
//! - Corn Field
//! - CornModel: Relation to Corn Model Entity
//! - Mesh3d: Holds corn model entities mesh, when loaded
//! 
//! ## Render World Corn Field Entity
//! - Corn Field
//! - Corn Model: updated with render entities
//! - Corn Lod Info: Cloned from the Corn Model Entity.
//! 
use std::hash::Hash;
use thiserror::Error;
use async_channel::Sender;
use bevy::{
    asset::{AsAssetId, AssetPath}, platform::collections::HashMap, prelude::*, render::{render_resource::Buffer, renderer::RenderDevice, sync_component::SyncComponentPlugin, sync_world::RenderEntity, Extract, Render, RenderApp, RenderSet}, tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task}
};
use wgpu::{util::BufferInitDescriptor, BufferUsages};
use crate::{ecs::corn::render::{ExtendWithCornMaterial, StdCornDrawRender, StdCornMaterial}, util::{event_set::{AppEventSet, EventSet}, extract_changed::ExtractChangedComponentPlugin, observer_ext::ObserveAsAppExt}};
use super::{CornField, CornFieldObserver, CornFieldSystemSet};

pub const DEFAULT_MODEL_PATH: &'static str = "models/corn3.glb";

/// Error type for when loading a corn mesh from a gltf fails.
#[derive(Default, Debug, Error)]
#[error("Corn meshes failed to merge together")]
pub struct ConvertCornMeshError;

/// Merges a set of meshes sorted into lods, into a single conglomerated mesh
async fn merge_meshes(meshes: Vec<Vec<Mesh>>) -> Result<Mesh, ConvertCornMeshError>{
    let mut lods: Vec<(usize, Vec<&Mesh>)> = meshes.iter().map(|lod| {
        let count = lod.iter().map(|mesh| mesh.count_vertices()).sum();
        (count, lod.iter().collect::<Vec<&Mesh>>())
    }).collect();
    lods.sort_by(|(a, _), (b, _)| {b.cmp(a)});
    let mut iter = lods.into_iter().map(|(_, lod)| lod.into_iter()).flatten();
    let mut merged = iter.next().unwrap().clone();
    for mesh in iter {
        merged.merge(mesh).map_err(|_| ConvertCornMeshError)?;
    }
    Ok(merged)
}

// Clones mesh data from a gltf asset and returns the mesh list and lod info
fn get_mesh_data(
    gltf: &Gltf, 
    scene_assets: &mut Assets<Scene>,
    mesh_assets: &Assets<Mesh>,
) -> Option<(Vec<Vec<Mesh>>, Vec<(usize, usize)>)> {
    // Get GLTF World
    let Some(scene) = gltf.scenes.get(0)
        .and_then(|scene| scene_assets.get_mut(scene)) 
    else {return None;};
    let world = &mut scene.world;
    // Find the meshes
    let mut mesh_query = world.query::<(&ChildOf, &Mesh3d)>();
    let meshes: Vec<(Handle<Mesh>, Entity)> = mesh_query.iter(world)
        .map(|(parent, mesh3d)| (mesh3d.0.clone(), parent.parent())).collect();
    // Group meshes by their parents parent. 
    let mut parent_query = world.query::<&ChildOf>();
    let mut lods: HashMap<Entity, Vec<Handle<Mesh>>> = HashMap::default();
    for (mesh, parent) in meshes.into_iter(){
        let Ok(middle) = parent_query.get(world, parent) else {continue;};
        if let Some(list) = lods.get_mut(&middle.parent()) {
            list.push(mesh);
        } else {
            lods.insert(middle.parent(), vec![mesh]);
        }
    }
    // Get Mesh Pointers
    let mut lods: Vec<(usize, Vec<&Mesh>)> = lods.into_values().map(|lod| {
        let mesh_pointers: Vec<&Mesh> = lod.into_iter().map(|handle| mesh_assets.get(&handle).unwrap()).collect();
        let size = mesh_pointers.iter().map(|mesh| mesh.count_vertices()).sum();
        (size, mesh_pointers)
    }).collect();
    // Sort lods
    lods.sort_by(|(a, _), (b, _)| b.cmp(a));
    // Get Index counts
    let vertex_counts: Vec<usize> = lods.iter().map(|(_, mesh)| {
        mesh.iter().map(|m| m.indices().unwrap().len()).sum()
    }).collect();
    let lod_data = vertex_counts.iter().fold((vec![], 0), | (mut lod_data, sum), val| {
        lod_data.push((*val, sum));
        (lod_data, sum+val)
    }).0;
    // Clone meshes
    let lods = lods.into_iter().map(|(_, lod)| lod.into_iter().cloned().collect()).collect();
    // Return meshes and lod info
    Some((lods, lod_data))
}

/*
    Corn Model Components
*/

/// Tag component for the default corn mesh entity
#[derive(Debug, Default, Clone, Copy, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct IsDefaultCornMesh;
impl IsDefaultCornMesh{
    /// Spawns the default corn model gltf
    pub fn spawn_default(world:&mut World){
        let assets = world.resource::<AssetServer>();
        let bundle = CornGltf::new_model_bundle(DEFAULT_MODEL_PATH, assets);
        world.spawn((Self, Name::from("Default Corn Model"), bundle));
    }
}

/// Holds the strong handle for the mesh asset of a corn model
#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)] #[component(immutable)]
pub struct CornMesh(pub Handle<Mesh>);
impl AsAssetId for CornMesh{
    type Asset = Mesh;
    fn as_asset_id(&self) -> AssetId<Self::Asset> {self.0.id()}
}

/// Holds the LOD info of a corn model. Vec of (total vtx count, start vtx index) per LOD
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
pub struct CornLodInfo(pub Vec<(usize, usize)>);

/// Relation target holding fields that use this corn model
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Component)]
#[reflect(Component)] #[relationship_target(relationship=CornModel)]
pub struct CornFields(Vec<Entity>);

/// Holds a strong handle to the gltf asset for a corn model
#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)] #[component(immutable)]
pub struct CornGltf(pub Handle<Gltf>);
impl AsAssetId for CornGltf{
    type Asset = Gltf;
    fn as_asset_id(&self) -> AssetId<Self::Asset> {self.0.id()}
}
impl CornGltf{
    /// Constructs a component bundle of a new corn model from a gltf path. Sets up the async mesh loading pipeline
    pub fn new_model_bundle<'a>(path: impl Into<AssetPath<'a>>, asset_server: &AssetServer) -> (Self, CornMesh, CornLoadSender){
        let gltf_handle: Handle<Gltf> = asset_server.load(path);
        let (tx, rx) = async_channel::bounded(1);
        let mesh_handle = asset_server.add_async(async move {
            let meshes = rx.recv().await.map_err(|_| ConvertCornMeshError)?;
            merge_meshes(meshes).await
        });

        (Self(gltf_handle), CornMesh(mesh_handle), CornLoadSender(tx))
    }
}

pub struct MaterialFromGltf;


/// Holds the sender used to give the asynchronous mesh load function the meshes of the corn model. 
#[derive(Debug, Clone, Component)]
pub struct CornLoadSender(pub Sender<Vec<Vec<Mesh>>>);
impl CornLoadSender{
    /// System which runs on GLTF asset event and sends loaded corn gltf data to async mesh load functions
    pub fn on_gltf_load(
        loading: Query<(Entity, &Self, &CornGltf, Has<MeshMaterial3d<StdCornMaterial>>)>,
        gltf_assets: Res<Assets<Gltf>>,
        scene_assets: ResMut<Assets<Scene>>,
        mesh_assets: Res<Assets<Mesh>>,
        material_assets: Res<Assets<StandardMaterial>>,
        mut extended_material_assets: ResMut<Assets<StdCornMaterial>>,
        mut commands: Commands
    ){
        let scene_assets = scene_assets.into_inner();
        let mesh_assets = mesh_assets.into_inner();
        for (entity, CornLoadSender(sender), CornGltf(gltf), has_material_already) in loading.iter(){
            let Some(asset) = gltf_assets.get(gltf) else {continue;};
            // Collect and sort meshes, and get lod info
            let Some((data, lod_info)) = get_mesh_data(asset, scene_assets, mesh_assets) else {continue;};
            // Send data to async load method
            let _ = sender.force_send(data); sender.close();

            if ! has_material_already{
                // get material from GLTF
                // TODO better asset pipelining
                let mut material = if let Some(handle) = asset.materials.get(0) {
                    info!("using material from GLTF {}", asset.named_materials.iter().find(|a|a.1 == handle).map(|a|a.0).cloned().unwrap_or_default() );
                    material_assets.get(handle).expect("material asset should be loaded").clone()
                } else {
                    warn!("Failed to get material from GLTF asset, using default material");
                    StandardMaterial::from_color(Srgba::GREEN)
                };

                // eas: turns out these were set correctly by blender. something else is wrong.
                // dbg!(material.double_sided, material.cull_mode);
                material.double_sided = true; // just to make sure, XXX not working, still false ???
                material.cull_mode = None; // does nothing because I hard code this in specialize for CornMaterial

                let material = material.extend_with_corn();
                let material = extended_material_assets.add(material); 
                commands.entity(entity).insert(MeshMaterial3d(material));
            }
            
            commands.entity(entity)
                .insert(CornLodInfo(lod_info))// Add lod info as component
                .remove::<CornLoadSender>();// Remove the corn load sender since we are finished with it
        }
    }
}

/// Holds the asynchronous task which merges an altered corn mesh into a new master mesh asset
#[derive(Debug, Component)]
pub struct CornModifyTask(pub Task<Result<(Vec<(usize, usize)>, Mesh), ConvertCornMeshError>>);
impl CornModifyTask{
    /// Creates a new async task which merges the meshes
    pub fn new(lod_info: Vec<(usize, usize)>, meshes: Vec<Vec<Mesh>>) -> Self{
        let task_pool = AsyncComputeTaskPool::get();
        let task = task_pool.spawn(async move {
            let mesh = merge_meshes(meshes).await?;
            Ok((lod_info, mesh))
        });
        Self(task)
    }
    /// System which runs on GLTF Asset Event, and sets up async merge tasks
    pub fn on_gltf_change(
        // All models which aren't loading, but have changed gltf asset data
        query: Query<(Entity, &CornGltf), (AssetChanged<CornGltf>, With<CornMesh>, Without<CornLoadSender>)>,
        gltf_assets: Res<Assets<Gltf>>,
        scene_assets: ResMut<Assets<Scene>>,
        mesh_assets: Res<Assets<Mesh>>,
        mut commands: Commands
    ){
        let mut batch = vec![];
        let scene_assets = scene_assets.into_inner();
        let mesh_assets = mesh_assets.into_inner();
        for (entity, CornGltf(gltf)) in query.into_iter(){
            // Clone data
            let Some(gltf) = gltf_assets.get(gltf) else {continue;};
            let Some((data, lod_info)) = get_mesh_data(gltf, scene_assets, mesh_assets) else {continue;};
            // Asynchronously merge the meshes in a task. Attach the task to the corn model entity.
            batch.push((entity, Self::new(lod_info, data)));
        }
        commands.insert_batch(batch);
    }
    /// System which replaces corn meshes and updates lod info when the merge task finishes
    pub fn poll_tasks(
        mut tasks: Query<(Entity, &mut Self, &CornMesh)>,
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>
    ){
        for (entity, mut task, CornMesh(mesh_id)) in tasks.iter_mut(){
            if !task.0.is_finished() {continue;}
            // Finish task
            let Some(Ok((lod_info, mesh))) = block_on(future::poll_once(&mut task.0)) else {continue;};
            // Update lod info, remove task component
            commands.entity(entity).insert(CornLodInfo(lod_info)).remove::<Self>();
            // Update mesh
            meshes.insert(mesh_id.id(), mesh);
        }
    }
}

/// Component of corn models which holds an indirect buffer with lod info stored in it
#[derive(Debug, Clone, Component)]
pub struct CornModelIndirectBuffer(pub Buffer, pub Vec<(usize, usize)>);
impl CornModelIndirectBuffer{
    /// Creates a new buffer from lod info
    pub fn new(lod_info: &Vec<(usize, usize)>, render_device: &RenderDevice) -> Self{
        let data: Vec<u32> = lod_info.iter().map(|(total, start)| 
            [*total as u32, 0, *start as u32, 0, 0]
        ).flatten().collect();

        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Corn Model Indirect Buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: BufferUsages::COPY_SRC
        });

        Self(buffer, lod_info.clone())
    }
    /// Runs during prepare resources, builds indirect buffers with this corn models lod info.
    pub fn create_buffers(
        models: Query<(Entity, &CornLodInfo, Option<&Self>), Or<(Changed<CornLodInfo>, Without<Self>)>>,
        render_device: Res<RenderDevice>,
        mut commands: Commands
    ){
        for (entity, CornLodInfo(lod_info), buffer) in models.iter(){
            if buffer.is_some_and(|b| b.1 == *lod_info) {continue;}
            let new_buffer = Self::new(lod_info, &render_device);
            commands.entity(entity).insert(new_buffer);
        }
    }
}

/*
    Corn Field Components
*/

/// Observer which attaches the default corn model to corn fields without a model assigned
pub fn attach_default_corn_model(
    trigger: Trigger<OnAdd, CornField>,
    default_mesh: Single<Entity, With<IsDefaultCornMesh>>,
    // mats: Query<&MeshMaterial3d<StdCornMaterial>>,
    mut commands: Commands
){
    let mut entity_commands = commands.entity(trigger.target());
    entity_commands.insert_if_new(CornModel(*default_mesh));

    // Material isn't loaded yet
    // let material = mats.get(*default_mesh).unwrap().clone();
    // entity_commands.insert_if_new(material);
}

pub fn attach_default_corn_material(
    query: Query<Entity, (With<CornField>, Without<MeshMaterial3d<StdCornMaterial>>)>,
    default_mesh: Option<Single<
        &MeshMaterial3d<StdCornMaterial>, 
        With<IsDefaultCornMesh>
    >>,
    mut commands: Commands
){
    let Some(material) = default_mesh else { return };
    for entity in query {
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert_if_new(material.clone());
    }
}

/// Relation to the corn model used by the field
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Component)]
#[reflect(Component)] #[relationship(relationship_target=CornFields)]
pub struct CornModel(pub Entity);
impl CornModel{
    /// Adds mesh3d when corn field is related to a corn model
    fn observe_on_insert(
        trigger: Trigger<OnInsert, Self>,
        fields: Query<&Self>,
        mut commands: Commands,
        models: Query<&CornMesh>
    ){
        let Ok(relation) = fields.get(trigger.target()) else {return;};
        let Ok(mesh) = models.get(relation.0) else {return;};
        commands.entity(trigger.target()).insert(Mesh3d(mesh.0.clone()));
    }
    
    /// Extracts this component to the render world, grabbing some corn model components as well.
    fn extract_component(
        mut commands: Commands,
        fields: Extract<Query<(RenderEntity, Ref<Self>)>>,
        models: Extract<Query<(RenderEntity, Ref<CornLodInfo>)>>,
        render_data: Query<(),(With<Self>, With<CornLodInfo>)>
    ){
        let mut relations = Vec::with_capacity(1);
        let mut lod_infos = Vec::with_capacity(1);
        // Adds CornModel as a render app corn field entity, with its entity pointer correctly updated, as well as adding CornLodInfo to the corn fields
        for (entity, related_model) in &fields {
            let Ok((model_entity, lod_info)) = models.get(related_model.0) else {continue;};
            let needs_insert = !render_data.get(entity).is_ok();
            if related_model.is_changed() || needs_insert{
                relations.push((entity, Self(model_entity)));
                lod_infos.push((entity, lod_info.clone()));
            } else if lod_info.is_changed() {
                lod_infos.push((entity, lod_info.clone()));
            }
        }
        commands.try_insert_batch(relations);
        commands.try_insert_batch(lod_infos);
    }
}

pub struct CornModelPlugin;
impl Plugin for CornModelPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<IsDefaultCornMesh>()
            .register_type::<CornGltf>()
            .register_type::<CornMesh>()
            .register_type::<CornLodInfo>()
            .register_type::<CornModel>()
            .register_type::<CornFields>()
            .add_plugins((
                ExtractChangedComponentPlugin::<CornMesh>::default(),
                ExtractChangedComponentPlugin::<CornLodInfo>::default(),
                SyncComponentPlugin::<CornModel>::default()
            ))
            .configure_event_set::<AssetEvent<Gltf>>(Update, EventSet::<AssetEvent<Gltf>>::default())
            .add_observer_as(attach_default_corn_model, CornFieldObserver)
            .add_observer_as(CornModel::observe_on_insert, CornFieldObserver)
            .add_systems(Update, attach_default_corn_material)
            .add_systems(Update, (
                (CornLoadSender::on_gltf_load, CornModifyTask::on_gltf_change).in_set(EventSet::<AssetEvent<Gltf>>::default()),
                CornModifyTask::poll_tasks
            ).in_set(CornFieldSystemSet))
        .sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, CornModel::extract_component)
            .add_systems(Render, CornModelIndirectBuffer::create_buffers.in_set(RenderSet::PrepareResources));
    }
    fn finish(&self, app: &mut App) {
        IsDefaultCornMesh::spawn_default(app.world_mut());
    }
}

