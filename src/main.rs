use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use corn_game::prelude::*;
/*

Use grave key to lock mouse and enable free cam movement
space/shift to go up and down
at the moment a corn field is either spawned or destroyed once per frame for testing purposes

*/
fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, WorldInspectorPlugin::new(), CornPlugin{}));
    app.run();
}