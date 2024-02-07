pub mod field;
pub mod buffer;
pub mod data_pipeline;
pub mod render;
pub mod asset;

use bevy::prelude::*;
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
            CornAssetPlugin
        )).init_resource::<CornCommonShader>()
        .add_systems(Startup, load_corn_common_shader);
    }
}

/// Adds the corn_common shader to the app. This is necessary in order for our other shaders, who import structs from corn_common, to be compiled
/// Bevy does not automatically recursively search for shader include files and load them, so we have to load them here
fn load_corn_common_shader(mut res: ResMut<CornCommonShader>, assets: Res<AssetServer>){
    res.0 = Some(assets.load::<Shader>("shaders/corn/corn_common.wgsl"));
}

#[derive(Default, Resource)]
pub struct CornCommonShader(pub Option<Handle<Shader>>);