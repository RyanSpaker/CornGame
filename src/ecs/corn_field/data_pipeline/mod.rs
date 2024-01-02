pub mod state_manager;
pub mod storage_manager;
pub mod operation_manager;
pub mod operation_executor;

use std::marker::PhantomData;
use bevy::{prelude::*, render::{RenderApp, Extract}};
use super::{corn_fields::simple_corn_field::{SimpleHexagonalCornField, SimpleRectangularCornField}, RenderableCornField};

/*
    Plugins:
*/

/// A plguin used to add a specific renderable corn field implementation into the game
/// Adds an extract function which queries the corn fields of type T, and adds them to the render app, 
/// along with a common component RenderableCornFieldID, which holds its hash id
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
        app.add_plugins((
            //state_manager::CornFieldStatePlugin::<T>::new(),
            operation_manager::CornOperationPlugin::<T>::new(),
            operation_executor::CornOperationExecutionPlugin::<T>::new()
        ));
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app.add_systems(ExtractSchedule, extract_renderable_corn_field::<T>);
        }
    }
}

/// A plugin that contains all single instance systems and data that corn fields need
/// Adds each master plugin, as well as each corn field specific plugin
pub struct MasterCornFieldDataPipelinePlugin;
impl Plugin for MasterCornFieldDataPipelinePlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((
            //state_manager::MasterCornFieldStatePlugin{},
            storage_manager::MasterCornStorageManagerPlugin{},
            operation_manager::MasterCornOperationPlugin{},
            operation_executor::MasterCornOperationExecutionPlugin{},
            RenderableCornFieldPlugin::<SimpleHexagonalCornField>::new(),
            RenderableCornFieldPlugin::<SimpleRectangularCornField>::new()
        ));
    }
}

/*
    Systems:
*/

/// This function runs during extract.
/// It runs once for every type that implements RenderableCornField, and has added a RenderableCornFieldPlugin<T> to the app.
/// It copies the component to the render_app entity, and adds a component RenderableCornFieldID with the components hash value
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

/*
    Tests:
*/


