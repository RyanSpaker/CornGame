use bevy::prelude::*;
use bevy::ecs::error::{GLOBAL_ERROR_HANDLER, error};

use corn_game::CornGame;

fn main() {
    GLOBAL_ERROR_HANDLER.set(error).expect("set once");

    let mut app = App::new();
    app.add_plugins(CornGame);
    app.run();
}
