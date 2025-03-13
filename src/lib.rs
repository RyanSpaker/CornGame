use app::CornAppPlugin;
use bevy::prelude::*;
use bevy_editor_pls::EditorPlugin;
use ecs::CornGameECSPlugin;

pub mod app;
pub mod ecs;
pub mod util;


pub struct CornGame;
impl Plugin for CornGame{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_plugins(
            DefaultPlugins
            .set(WindowPlugin{
                    primary_window: Some(Window { 
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
            .set(AssetPlugin{
                    mode: AssetMode::Processed,
                    ..default()
                }
            )
            // .set(bevy::log::LogPlugin {
            //     level: bevy::log::Level::DEBUG,
            //     filter: "debug,wgpu_hal=error,wgpu_core=error,corn_game=debug".to_string(),
            //     ..default()
            // })
        );
        app.add_plugins((
            CornAppPlugin,
            CornGameECSPlugin
        ));
    }
}