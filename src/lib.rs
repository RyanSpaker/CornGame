use std::{path::PathBuf, time::Duration};

use bevy::{app::ScheduleRunnerPlugin, asset::io::AssetSourceBuilder, prelude::*, render::{view::RenderLayers, RenderPlugin}};
use bevy_editor_pls::bevy_inspector_egui::bevy_inspector::hierarchy::Hierarchy;
use clap::Parser;

pub mod ecs;
pub mod scenes;
pub mod systems;
pub mod util;

use serde::{Deserialize, Serialize};
use util::debug_app::DebugApp;
use we_clap::WeParser;

use crate::{ecs::test_cube::TestCube, systems::{network::CornNetworkingPlugin, physics::CornPhysicsPluginNetworkPlugin}, util::propogate::HierarchyPropagatePlugin};

impl we_clap::WeParser for Cli {}
#[derive(Debug, Clone, clap::Parser, Default, Reflect, Serialize, Deserialize, Resource)]
#[reflect(Resource)]
struct Cli {
    /// implies --lobby
    scenes: Vec<PathBuf>,
    #[arg(short, long)]
    client: bool,
    #[arg(short, long)]
    server: bool,

    #[arg(short, long)]
    menu: bool,

    #[arg(short, long)]
    lobby: bool,

    #[arg(long)]
    headless: bool,

    /// auto interact and (TODO) walk around
    #[arg(long)]
    dummy: bool,

    /// spawn a test cube
    #[arg(long)]
    testcube: bool,
}

#[derive(Debug, Resource)]
pub struct Headless;

pub struct CornGame;
impl Plugin for CornGame {
    fn build(&self, app: &mut bevy::prelude::App) {
        let cli: Cli = Cli::we_parse();
        app.insert_resource(cli.clone());
        // fn custom_layer(app: &mut App) -> Option<BoxedLayer> {
        //     let logger = TracingDynamicSubscriber::default();
        //     app.insert_resource(logger.clone());
        //     Some(logger.boxed())
        // }

        app.register_asset_source(
            "shaders", // The unique name for your source
            AssetSourceBuilder::platform_default("shaders", None),
        );

        let mut pg = DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: bevy::window::PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                // setting these for wasm, if we want to use the asset preprocessor we should change this
                mode: AssetMode::Unprocessed,
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            });

        if cli.headless {
            // pg = pg.disable::<RenderPlugin>();
            app.insert_resource(Headless);
            pg = pg
                // Not strictly necessary, as the inclusion of ScheduleRunnerPlugin below
                // replaces the bevy_winit app runner and so a window is never created.
                .set(WindowPlugin {
                    primary_window: None,
                    // Don’t automatically exit due to having no windows.
                    // Instead, the code in `update()` will explicitly produce an `AppExit` event.
                    exit_condition: bevy::window::ExitCondition::DontExit,
                    ..default()
                })
                // WinitPlugin will panic in environments without a display server.
                .disable::<bevy::winit::WinitPlugin>();
            app.add_plugins(pg);
            app.add_plugins(ScheduleRunnerPlugin::run_loop(
                // Run 60 times per second.
                Duration::from_secs_f64(1.0 / 60.0),
            ));
            app.add_plugins((
                CornNetworkingPlugin, 
            ));

        }else {
            // .set(LogPlugin {
            //     level: bevy::log::Level::TRACE,
            //     filter: "info,wgpu_hal=error,wgpu_core=error,corn_game=debug".to_string(),
            //     custom_layer,
            //     ..Default::default()
            // }),
            //.disable::<LogPlugin>(),

            app.add_plugins(pg);
            //app.add_plugins(bevy_editor_pls::default_windows::utils::log_plugin::LogPlugin::default());

            app.add_plugins((
                bevy_ui_text_input::TextInputPlugin,
                bevy_flair::FlairPlugin,
                systems::CornSystemsPlugin,
                scenes::CornScenesPlugin,
                ecs::CornECSPlugin,
                bevy_skein::SkeinPlugin::default(),
                bevy_mod_skinned_aabb::SkinnedAabbPlugin::default(), // fixes issue with frustum cull of animated objects by recomputing aabb every frame, minor perf hit supposedly
            ));

            app.add_debug_plugins((
                crate::util::desync_ids::DebugWarnRenderId,
                // bevy::remote::RemotePlugin::default(),
                // bevy::remote::http::RemoteHttpPlugin::default(),
            ));
        }

        // TODO: needed for menu, but where to put this?
        // we have the annoying fact that this is needed in multiple places and will fail silently if this plugin isn't added
        app.add_plugins(
            HierarchyPropagatePlugin::<RenderLayers>::default()
        );
    }
}
