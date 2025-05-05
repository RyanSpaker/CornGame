pub mod util;
pub mod scenes;
pub mod audio;
pub mod network;
pub mod ui;
pub mod character;
pub mod physics;
pub mod interactions;

use bevy::{pbr::FogVolume, prelude::*};
use bevy_edge_detection::EdgeDetectionPlugin;
use blenvy::BlenvyPlugin;

pub struct CornSystemsPlugin;
impl Plugin for CornSystemsPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<FogVolume>()
            .add_plugins((
                util::AppUtilPlugin, 
                scenes::SceneTransition,
                physics::CornPhysicsPlugin,
                audio::CornAudioPlugin,
                //ui::editor::MyEditorPlugin,
                network::CornNetworkingPlugin,
                //character::MyCharacterPlugin, 
                //interactions::InteractPlugin,
               
            ))
            .add_plugins((
                BlenvyPlugin::default(),
                EdgeDetectionPlugin::default()
            ));
    }
}