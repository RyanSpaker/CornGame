pub mod cf_simple;
pub mod cf_image_carved;
pub mod state;

use std::marker::PhantomData;
use bevy::{
    prelude::*,
    reflect::GetTypeRegistration,
    render::{sync_world::RenderEntity, Extract, RenderApp}
};
use blenvy::GltfComponentsSet;
use cf_image_carved::ImageCarvedHexagonalCornField;
use cf_simple::{SimpleHexagonalCornField, SimpleRectangularCornField};
use serde::Deserialize;
use state::{CornAssetState, MasterCornFieldStatePlugin};
use wgpu::core::device::resource;
use self::state::CornFieldStatePlugin;
use super::data_pipeline::{operation_executor::{IntoCornPipeline, IntoOperationResources}, operation_manager::IntoBufferOperation, CornFieldPipelinePlugin};

pub mod prelude{
    pub use super::cf_simple::{SimpleHexagonalCornField, SimpleRectangularCornField};
    pub use super::cf_image_carved::ImageCarvedHexagonalCornField;
}


/// This trait represents all implementation specific corn field settings
/// Impl this trait to create a type of corn field
/// Make sure to add a RenderableCornFieldPlugin<T> to the app as well
pub trait RenderableCornField: Component + Clone + GetTypeRegistration + std::fmt::Debug + CornAssetState + IntoBufferOperation + IntoOperationResources + IntoCornPipeline{
    /// This function returns a hash of the component used for an ID. 
    /// The hash value should use any values that change the structure of the corn, and none others.
    /// Any time the hash is changed, the corn is deleted off the gpu and re-initialized.
    /// Every unique corn field is expected to have a unique hash as well, so make sure the hashes use enough unique values
    fn gen_id(&self) -> RenderableCornFieldID;
    /// This function can be overridden to add custom functionality for each corn field type
    fn add_functionality(_app: &mut App){}
}

/// This component holds the hash id for a single corn field in a recognizable struct.
/// This makes it so that you can query all corn fields in the render app at once
#[derive(Debug, Clone, Hash, PartialEq, Eq, Component)]
pub struct RenderableCornFieldID{
    id: u64
}
impl From<u64> for RenderableCornFieldID{
    fn from(value: u64) -> Self {
        Self{id: value}
    }
}


/// Function responsible for extracting corn field components to the RenderApp
pub fn extract_renderable_corn_field<T: RenderableCornField>(
    mut commands: Commands,
    mut previous_len: Local<usize>,
    query: Extract<Query<(RenderEntity, &T)>>
){
    let mut values = Vec::with_capacity(*previous_len);
    for (entity, query_item) in &query {
        values.push((entity, (query_item.clone(), query_item.gen_id())));
    }
    *previous_len = values.len();
    commands.insert_or_spawn_batch(values);
}


pub struct RenderableCornFieldPlugin<T: RenderableCornField>{
    _marker: PhantomData<T>
}
impl<T: RenderableCornField> RenderableCornFieldPlugin<T>{
    pub fn new() -> Self {
        RenderableCornFieldPlugin { _marker: PhantomData::<T> }
    }
}
impl<T: RenderableCornField> Plugin for RenderableCornFieldPlugin<T>{
    fn build(&self, app: &mut App) {
        app
            .register_type::<T>()
        .sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, extract_renderable_corn_field::<T>);
        T::add_functionality(app);
        app.add_plugins((
            CornFieldStatePlugin::<T>::new(),
            CornFieldPipelinePlugin::<T>::new()
        ));
    }
}

#[derive(Component, Reflect, Default, Debug, Deserialize)]
#[reflect(Component)]
/// test component for loading cornfields from blender
pub struct CornTestGltf(String);

fn init_gltf_cornfield(
    corn: Query<(Entity, &CornTestGltf, &Children,  &GlobalTransform), Without<ImageCarvedHexagonalCornField>>,
    mut children: Query<(&MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    a_materials: Res<Assets<StandardMaterial>>,
    mut commands: Commands
){
    for (id, _corn, child, transform) in corn.iter() {
        info!("initializing gltf loaded cornfield entity {}", id);

        let (h_mat, mut visible) = children.get_mut(*child.first().unwrap()).unwrap();

        // hide the reference plane
        *visible = Visibility::Hidden;

        let Some(material) = a_materials.get(h_mat) else { break };
        let h_image = material.base_color_texture.clone().unwrap();

        // NOTE: we use the transform of corn object. 
        // This means that you CANNOT apply the transform in blender
        // we fully assume the plane of the model is 1x1
        // NOTE: rotation not supported yet.
        // TODO: actually use the mesh in corn render.
        // TODO: should use global transform

        let transform = transform.compute_transform();
        let center = transform.translation + Vec3::new(0.0, 0.0, 0.0); 

        let half_extents = transform.scale.xz();
        dbg!(half_extents, center);

        commands.entity(id).insert(
            ImageCarvedHexagonalCornField::new(
                center, half_extents, 
                0.75, Vec2::new(1.6, 1.8), 0.2, 
                h_image,
            )
        );
    }
}

pub struct CornFieldsPlugin;
impl Plugin for CornFieldsPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((
            RenderableCornFieldPlugin::<SimpleHexagonalCornField>::new(),
            RenderableCornFieldPlugin::<SimpleRectangularCornField>::new(),
            RenderableCornFieldPlugin::<ImageCarvedHexagonalCornField>::new(),
            MasterCornFieldStatePlugin
        ));

        app.add_systems(Update, init_gltf_cornfield.after(GltfComponentsSet::Injection) );

        app.register_type::<CornTestGltf>(); // needed for loading from gltf
    }
}