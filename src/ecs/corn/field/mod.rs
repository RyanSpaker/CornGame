pub mod cf_simple;
pub mod cf_image_carved;
pub mod state;

use std::marker::PhantomData;
use bevy::{
    prelude::*,
    reflect::GetTypeRegistration,
    render::{Extract, RenderApp}
};
use cf_image_carved::ImageCarvedHexagonalCornField;
use cf_simple::{SimpleHexagonalCornField, SimpleRectangularCornField};
use state::{CornAssetState, MasterCornFieldStatePlugin};
use self::state::CornFieldStatePlugin;
use super::data_pipeline::{operation_executor::{IntoCornPipeline, IntoOperationResources}, operation_manager::IntoBufferOperation, CornFieldPipelinePlugin};


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
    query: Extract<Query<(Entity, &T)>>
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


pub struct CornFieldsPlugin;
impl Plugin for CornFieldsPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((
            RenderableCornFieldPlugin::<SimpleHexagonalCornField>::new(),
            RenderableCornFieldPlugin::<SimpleRectangularCornField>::new(),
            RenderableCornFieldPlugin::<ImageCarvedHexagonalCornField>::new(),
            MasterCornFieldStatePlugin
        ));
    }
}