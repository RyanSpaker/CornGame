pub mod corn_model;

use bevy::prelude::*;

pub struct CornAssetPlugin{}
impl Plugin for CornAssetPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_state::<corn_model::CornLoadState>();
    }
}