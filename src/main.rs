use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use corn_game::prelude::*;
/*

Use grave key to lock mouse and enable free cam movement
space/shift to go up and down

*/
fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, WorldInspectorPlugin::new(), CornPlugin{}));
    app.run();
}