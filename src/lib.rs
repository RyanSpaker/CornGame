use bevy::prelude::*;

pub mod systems;
pub mod scenes;
//pub mod ecs;
pub mod util;

pub struct CornGame;
impl Plugin for CornGame{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((
            DefaultPlugins.set(WindowPlugin{
                primary_window: Some(Window { 
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }).set(AssetPlugin{
                mode: AssetMode::Processed,
                ..Default::default()
            }),
            systems::CornSystemsPlugin,
            scenes::CornStatesPlugin,
           // ecs::CornECSPlugin
        ));
    }
}