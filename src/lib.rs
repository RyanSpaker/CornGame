use app::CornAppPlugin;
use bevy::prelude::*;
use ecs::CornGameECSPlugin;

pub mod app;
pub mod ecs;
pub mod util;


pub struct CornGame;
impl Plugin for CornGame{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins((
            DefaultPlugins.set(WindowPlugin{
                primary_window: Some(Window { 
                    present_mode: bevy::window::PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }).set(AssetPlugin{
                mode: AssetMode::Processed,
                ..Default::default()
            }),
            CornAppPlugin,
            CornGameECSPlugin
        ));
    }
}