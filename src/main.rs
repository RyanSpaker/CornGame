use bevy::prelude::*;
use corn_game::CornGame;
/*

Use grave key to lock mouse and enable free cam movement
space/shift to go up and down

There should be a window in the top left which you can open and then navigate to resources, and find a resource calld LOD Cutoffs
You need to manually set the values to something logical (i usually do 5, 10, 25, 50, 100, 250).

There is also a resource called FPS which you can open to view the current FPS of the application

*/

fn main() {
    let mut app = App::new();
    app.add_plugins(CornGame);
    app.run();
}