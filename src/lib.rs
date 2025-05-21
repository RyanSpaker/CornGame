pub mod systems;
pub mod scenes;
pub mod ecs;
pub mod util;

use std::path::PathBuf;
use bevy::{log::LogPlugin, prelude::*, render::{sync_world::RenderEntity, RenderApp}};
use clap::Parser;
use serde::{Deserialize, Serialize};

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
            ecs::CornECSPlugin
        ));

        app.insert_resource(Cli::parse());
        app.sub_app_mut(RenderApp)
            .add_systems(Startup, crank_render_generations);
        app.add_systems(Update, warn_synced_ids);
    }
}

/// deliberately desync render world and main world Entities to catch migration bugs faster.
/// XXX have not been able to confirm this makes any difference
fn crank_render_generations(world: &mut World) {
    let v: Vec<_> = (0..100).map(|_| world.spawn(()).id()).collect();
    for e in v {
        world.despawn(e);
    }
}

fn warn_synced_ids(query: Query<(Entity, &RenderEntity), Changed<RenderEntity>>) {
    for (id, r_id) in query.iter() {
        if id == r_id.id() {
            warn!("render world id's sync'd for {} don't rely on this", id);
        }
    }
}
