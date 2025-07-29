pub mod audio;
pub mod character;
pub mod interactions;
pub mod network;
pub mod physics;
pub mod scenes;
pub mod ui;
pub mod util;
pub mod animation_context;

use bevy::{pbr::FogVolume, prelude::*};

pub struct CornSystemsPlugin;
impl Plugin for CornSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<FogVolume>().add_plugins((
            ui::editor::MyEditorPlugin,
            util::AppUtilPlugin,
            scenes::SceneTransition,
            physics::CornPhysicsPlugin,
            audio::CornAudioPlugin,
            network::CornNetworkingPlugin,
            character::MyCharacterPlugin,
            interactions::InteractPlugin,
            animation_context::plugin,
        ));
        // TODO reimplement edge detection
        // .add_plugins((BlenvyPlugin::default(), EdgeDetectionPlugin::default()));
    }
}
