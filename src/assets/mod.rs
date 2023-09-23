pub mod corn_model;
pub mod shader_includes;

use bevy::prelude::*;
use shader_includes::ShaderIncludesPlugin;

pub struct CornAssetPlugin{}
impl Plugin for CornAssetPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_plugins(ShaderIncludesPlugin{})
            .add_state::<corn_model::CornLoadState>();
    }
}