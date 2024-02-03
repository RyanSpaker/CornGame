pub mod field;
pub mod buffer;
pub mod data_pipeline;
pub mod render;
pub mod asset;

use bevy::prelude::*;
use crate::core::CornGameState;

use self::{asset::CornAssetPlugin, buffer::CornBufferPlugin, data_pipeline::CornPipelinePlugin, field::CornFieldsPlugin, render::CornRenderPipelinePlugin};

/// Plugin that adds all of the corn field component functionality to the game
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CornFieldsPlugin, 
            CornBufferPlugin,
            CornPipelinePlugin,
            CornRenderPipelinePlugin,
            CornAssetPlugin::new(CornGameState::Init)
        ));
    }
}