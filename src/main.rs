use bevy::prelude::*;
use corn_game::CornGame;
/*
Use grave key to lock mouse and enable free cam movement
space/shift to go up and down
*/
fn main() {
    let mut app = App::new();
    app.add_plugins(CornGame);
    app.run();
}