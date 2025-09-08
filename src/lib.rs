pub mod ecs;
pub mod systems;
pub mod util;

use std::{path::PathBuf, time::Duration};
use bevy::{app::ScheduleRunnerPlugin, prelude::*, window::PresentMode};
use serde::{Deserialize, Serialize};
use util::debug_app::DebugApp;
use systems::network::CornNetworkingPlugin;
use crate::systems::scenes::prelude::*;

pub mod prelude{
    pub use super::{CornGameAppAPI, DevConfig};
}

/// CLI parsable config values used for development code and testing
#[derive(Debug, Clone, clap::Parser, Default, Reflect, Serialize, Deserialize, Resource)]
#[reflect(Resource)]
pub struct DevConfig {
    #[arg(short, long)]
    pub dummy: bool,
    #[arg(short, long)]
    pub server: bool,
    #[arg(short, long)]
    pub client: bool,

    pub scenes: Vec<PathBuf>,
}
impl we_clap::WeParser for DevConfig {}

pub trait CornGameAppAPI{
    fn setup_game_headless(&mut self) -> &mut Self;
    fn setup_game(&mut self, vsync: bool) -> &mut Self;
    fn add_corn_debug_plugins(&mut self) -> &mut Self;
    fn insert_dev_config(&mut self, config: DevConfig) -> &mut Self;
    fn get_scene_list(&mut self) -> Vec<String>;
    fn get_test_list(&mut self) -> Vec<String>;
}
impl CornGameAppAPI for App{
    /// Configures the app to run as a headless networking server
    fn setup_game_headless(&mut self) -> &mut Self{
        self.add_plugins(DefaultPlugins
            // Not strictly necessary, as the inclusion of ScheduleRunnerPlugin below
            // replaces the bevy_winit app runner and so a window is never created.
            .set(WindowPlugin{
                primary_window: None, 
                // Don’t automatically exit due to having no windows.
                // Instead, the code in `update()` will explicitly produce an `AppExit` event.
                exit_condition: bevy::window::ExitCondition::DontExit, 
                ..default()
            })
            .set(AssetPlugin{
                // setting these for wasm, if we want to use the asset preprocessor we should change this
                // For headless servers do we even need this?
                mode: AssetMode::Unprocessed,
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            })
            // WinitPlugin will panic in environments without a display server.
            .disable::<bevy::winit::WinitPlugin>()
        ).add_plugins(ScheduleRunnerPlugin::run_loop(
            // Run 60 times per second.
            Duration::from_secs_f64(1.0 / 60.0),
        )).add_plugins(CornNetworkingPlugin)
    }

    /// Configures the app to run as a normal graphical application.
    fn setup_game(&mut self, vsync: bool) -> &mut Self{
        self.add_plugins(DefaultPlugins
            .set(WindowPlugin{
                primary_window: Some(Window {
                    present_mode: if vsync {PresentMode::AutoVsync} else {PresentMode::AutoNoVsync},
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                // setting these for wasm, if we want to use the asset preprocessor we should change this
                mode: AssetMode::Unprocessed,
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            })
        ).add_plugins(
            bevy_skein::SkeinPlugin::default()
        ).add_plugins((
            systems::CornSystemsPlugin,
            ecs::CornECSPlugin,
        ))
    }

    /// Adds debug plugins and systems to the app
    fn add_corn_debug_plugins(&mut self) -> &mut Self{
        self.add_debug_plugins((
            crate::util::desync_ids::DebugWarnRenderId,
            // bevy::remote::RemotePlugin::default(),
            // bevy::remote::http::RemoteHttpPlugin::default(),
        ))
    }

    /// Add dev config struct to the app 
    fn insert_dev_config(&mut self, config: DevConfig) -> &mut Self {
        self.register_type::<DevConfig>().insert_resource(config)
    }
    
    /// Returns the list of all embedded scenes currently cached
    fn get_scene_list(&mut self) -> Vec<String> {
        self.list_scenes()
    }
    
    /// Returns the list of all embedded scenes currently cached
    fn get_test_list(&mut self) -> Vec<String> {
        self.list_tests()
    }
}
