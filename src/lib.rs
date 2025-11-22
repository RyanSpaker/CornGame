#![feature(arbitrary_self_types)]

use std::{path::PathBuf, sync::atomic::AtomicUsize, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin,
    asset::io::AssetSourceBuilder,
    ecs::schedule::ScheduleLabel,
    platform::collections::HashMap,
    prelude::*,
    remote::{
        builtin_methods::export_registry_types,
        schemas::json_schema::{JsonSchemaBevyType, export_type},
    },
    render::{
        RenderApp,
        RenderPlugin,
        diagnostic::RenderDiagnosticsPlugin,
        settings::WgpuSettings,
        view::RenderLayers,
    },
    scene::ron,
};
use bevy_dog::plugin;
use bevy_editor_pls::bevy_inspector_egui::bevy_inspector::hierarchy::Hierarchy;
use clap::Parser;

pub mod ecs;
pub mod scenes;
pub mod systems;
pub mod util;

use serde::{Deserialize, Serialize};
use util::debug_app::DebugApp;
use we_clap::WeParser;
use wgpu::Features;

use crate::{
    ecs::test_cube::TestCube,
    systems::{network::CornNetworkingPlugin, physics::CornPhysicsPluginNetworkPlugin},
    util::propogate::HierarchyPropagatePlugin,
};

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

    /// spawn a test cornfield
    #[arg(long)]
    testcorn: Option<f32>,

    /// disable fancy camera features like bloom
    #[arg(long)]
    simplecam: bool,

    #[arg(long)]
    spatialaudio: bool,

    /// export type registry and exit
    #[arg(long)]
    export_registry: bool,

    /// item to attach to player (also spawns player)
    #[arg(long)]
    item: Vec<String>,

    /// run system, see: util/register_system_named.rs
    #[arg(long)]
    system: Vec<String>,

    /// broken
    #[arg(long)]
    list_systems: bool,

    // whether to spawn wind sounds
    #[arg(long)]
    wind: bool,

    #[arg(env, long)]
    no_vsync: bool,
}

#[derive(Debug, Resource)]
pub struct Headless;

/// schedule to store systems which are runnable from the cli
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Cmds;

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

        app.init_schedule(Cmds);

        app.register_asset_source(
            "shaders", // The unique name for your source
            AssetSourceBuilder::platform_default("shaders", None),
        );

        let mut pg = DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: match cli.no_vsync {
                        false => bevy::window::PresentMode::AutoVsync,
                        true => bevy::window::PresentMode::Immediate, // ??? this doesn't seem to do anything
                    },
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
            .set(RenderPlugin {
                // enable performance metrics 
                // pretty sure I don't actually have to specify this and they get turned on by default
                render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings {
                    features: Features::TIMESTAMP_QUERY
                        | Features::PIPELINE_STATISTICS_QUERY
                        | WgpuSettings::default().features,
                    ..Default::default()
                }),
                ..Default::default()
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
            app.add_plugins((CornNetworkingPlugin,));
        } else {
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
                                                                     // systems::renderdoc::RenderDocPlugin::new_with_trigger_key(KeyCode::F10),
            ));

            // gpu profiling features, edit: already added by something.
            // app.add_plugins(RenderDiagnosticsPlugin);

            app.add_debug_plugins((
                crate::util::desync_ids::DebugWarnRenderId,
                // bevy::remote::RemotePlugin::default(),
                // bevy::remote::http::RemoteHttpPlugin::default(),
            ));
        }

        // app.add_plugins(StressTestPlugin);
        
        // TODO: needed for menu, but where to put this?
        // we have the annoying fact that this is needed in multiple places and will fail silently if this plugin isn't added
        app.add_plugins(HierarchyPropagatePlugin::<RenderLayers>::default());
    }

    fn finish(&self, app: &mut App) {
        let cli = app.world().resource::<Cli>();
        if cli.export_registry || std::env::var("CORN_EXPORT_REGISTRY").is_ok() {
            /// uses same function as BRP
            let registry = app.world().resource::<AppTypeRegistry>();
            let type_registry: HashMap<String, JsonSchemaBevyType> =
                registry.read().iter().map(export_type).collect();
            let serialized = serde_json::to_string_pretty(&type_registry)
                .expect("Failed to serialize type registry");
            std::fs::write("type_registry.json", serialized)
                .expect("Failed to write type registry");
            if cli.export_registry {
                // if using cli flag, just write json and exit
                std::process::exit(0);
            }
        }
    }
}


/// Test to see effect of lots of do-nothing systems on performance
/// prints number of times test_system ran to confirm system isn't getting deduped
struct StressTestPlugin;
impl Plugin for StressTestPlugin {
    fn build(&self, app: &mut App) {
        #[derive(bevy::ecs::schedule::ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
        struct StressTest;
        let mut schedule = Schedule::new(StressTest);
        // schedule.set_executor_kind(bevy::ecs::schedule::ExecutorKind::SingleThreaded);
        app.add_schedule(schedule);
        let mut main_schedule_order = app.world_mut().resource_mut::<bevy::app::MainScheduleOrder>();
        main_schedule_order.insert_after(Update, StressTest);

        #[derive(Debug, Event)]
        struct MyEvent;

        #[derive(Debug, Reflect, Default, Resource)]
        struct MyCount(AtomicUsize);

        app.init_resource::<MyCount>();
        app.add_event::<MyEvent>();

        fn test_system(_: Query<&Name>, num: Res<MyCount>, mut events: EventReader<MyEvent>){
            if events.is_empty(){
                return;
            }
            num.0.fetch_add(events.read().count(), std::sync::atomic::Ordering::Relaxed);
        }
        for _ in 0..10000 {
            app.add_systems(StressTest, test_system);
            // app.add_systems(StressTest, test_system.run_if(|local: Local<bool>| *local));
        }
        
        app.add_systems(Update, |
            mut events: EventWriter<MyEvent>,
            time: Res<Time>,
            mut ran: Local<bool>,
            mut printed: Local<bool>,
            count: Res<MyCount>,
        |{
            if !*ran && time.elapsed().as_secs_f32() > 3.0 {
                events.write(MyEvent);
                *ran = true;
            }
            else if *ran && !*printed {
                info!("test_system ran {} times", count.0.load(std::sync::atomic::Ordering::Relaxed));
                *printed = true;
            }
        });
    }
} 
