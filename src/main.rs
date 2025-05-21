use bevy::prelude::*;
use corn_game::{util::debug_app::DebugApp, CornGame};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
/*
Use grave key to lock mouse and enable free cam movement
space/shift to go up and down
*/
fn main() {
    let mut app = App::new();
    app.add_plugins(CornGame);
    app.add_debug_plugins((
        WorldInspectorPlugin::default(),
        bevy::remote::RemotePlugin::default(),
        bevy::remote::http::RemoteHttpPlugin::default(),
        bevy_remote_inspector::RemoteInspectorPlugins
    ));
    app.run();
}