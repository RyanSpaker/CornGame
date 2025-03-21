use std::path::PathBuf;

use app::CornAppPlugin;
use bevy::{prelude::*, reflect};
use bevy_editor_pls::EditorPlugin;
use clap::Parser;
use ecs::CornGameECSPlugin;
use lightyear::prelude::AppMessageExt;
use serde::{Deserialize, Serialize};

pub mod app;
pub mod ecs;
pub mod util;

#[derive(Debug, clap::Parser, Default, Reflect, Serialize, Deserialize, Resource)]
#[reflect(Resource)]
struct Cli {
    scenes: Vec<PathBuf>,

    #[arg(short, long)]
    client: bool,
    #[arg(short, long)]
    server: bool,
}

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

        app.insert_resource(Cli::parse());
    }
}