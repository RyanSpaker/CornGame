pub mod operation_manager;
pub mod operation_executor;

use std::marker::PhantomData;
use bevy::prelude::*;
use super::field::RenderableCornField;

/*
    Plugins:
*/

/// A plugin which adds corn pipeline functionality for each type of renderable corn field
pub struct CornFieldPipelinePlugin<T: RenderableCornField>{
    _marker: PhantomData<T>
}
impl<T: RenderableCornField> CornFieldPipelinePlugin<T>{
    pub fn new() -> Self {
        CornFieldPipelinePlugin { _marker: PhantomData::<T> }
    }
}
impl<T: RenderableCornField> Plugin for CornFieldPipelinePlugin<T>{
    fn build(&self, app: &mut App) {
        app.add_plugins((
            operation_manager::CornFieldOperationPlugin::<T>::new(),
            operation_executor::CornFieldOperationExecutionPlugin::<T>::new()
        ));
    }
}

/// A plugin that adds all universal corn pipeline functionality
pub struct CornPipelinePlugin;
impl Plugin for CornPipelinePlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((
            operation_manager::CornOperationPlugin,
            operation_executor::CornOperationExecutionPlugin
        ));
    }
}

