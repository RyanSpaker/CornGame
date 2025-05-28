use std::path::PathBuf;

use bevy::{
    log::{
        tracing_subscriber::{Layer, Registry},
        BoxedLayer,
        LogPlugin,
    },
    prelude::*,
    reflect,
    render::{sync_world::RenderEntity, RenderApp},
};
use bevy_editor_pls::{
    default_windows::{logging::TracingDynamicSubscriber},
};
use clap::Parser;
use lightyear::prelude::AppMessageExt;

pub mod ecs;
pub mod scenes;
pub mod systems;
pub mod util;

use serde::{Deserialize, Serialize};
use util::debug_app::DebugApp;

#[derive(Debug, clap::Parser, Default, Reflect, Serialize, Deserialize, Resource)]
#[reflect(Resource)]
struct Cli {
    scenes: Vec<PathBuf>,
    #[arg(short, long)]
    client: bool,
    #[arg(short, long)]
    server: bool,

    #[arg(short, long)]
    menu: bool,
}

pub struct CornGame;
impl Plugin for CornGame {
    fn build(&self, app: &mut bevy::prelude::App) {
        // fn custom_layer(app: &mut App) -> Option<BoxedLayer> {
        //     let logger = TracingDynamicSubscriber::default();
        //     app.insert_resource(logger.clone());
        //     Some(logger.boxed())
        // }

        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        present_mode: bevy::window::PresentMode::AutoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    mode: AssetMode::Processed,
                    ..default()
                })
                // .set(LogPlugin {
                //     level: bevy::log::Level::TRACE,
                //     filter: "info,wgpu_hal=error,wgpu_core=error,corn_game=debug".to_string(),
                //     custom_layer,
                //     ..Default::default()
                // }),
                //.disable::<LogPlugin>(),
        );
        //app.add_plugins(bevy_editor_pls::default_windows::utils::log_plugin::LogPlugin::default());
        
        app.add_plugins((
            systems::CornSystemsPlugin,
            scenes::CornScenesPlugin,
            ecs::CornECSPlugin,
        ));

        app.insert_resource(Cli::parse());
        app.add_debug_plugins((
            crate::util::desync_ids::DebugWarnRenderId,
            bevy_skein::SkeinPlugin::default(),
            // bevy::remote::RemotePlugin::default(),
            // bevy::remote::http::RemoteHttpPlugin::default(),
        ));
    }
}
