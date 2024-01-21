pub mod field;
pub mod buffer;
pub mod data_pipeline;
pub mod render;

use bevy::prelude::*;
use self::{buffer::CornBufferPlugin, data_pipeline::CornPipelinePlugin, field::CornFieldsPlugin, render::CornRenderPipelinePlugin};

/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CornFieldsPlugin, 
            CornBufferPlugin,
            CornPipelinePlugin,
            CornRenderPipelinePlugin
        ));
    }
}