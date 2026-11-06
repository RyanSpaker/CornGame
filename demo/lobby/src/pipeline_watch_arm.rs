//! CLAUDE (claude-sonnet-5-5) 2026-11-05: when the lobby counts as "loaded" for pipeline_watch.
//!
//! Policy lives here, separate from the detectors: the player exists, the authored scene has fully spawned
//! (corn_scene's `Ready` on a scene file's root), and, on web, the player has clicked in (pointer lock; on
//! native `browser::captured()` is always true). Everything pipeline-related before that is loading.
//!
//! TODO: this is a one-shot startup gate: the first scene's `Ready` arms the watcher once, and pipeline_watch
//! latches "precompile caught up" once. Scenes that spawn later (not at game start), or while other scenes
//! are still loaded, are not covered: their meshes' pipelines are requested at spawn, but they may be drawn
//! the same frame, and the watcher would report that as a miss, or stay silent if it hasn't latched yet.
//! To extend: track loading per scene instead of per game: count scene roots that have spawned but not yet
//! reported `Ready`; tag each `PipelineRequest` in precompile.rs with the scene root (or a load generation)
//! it was spawned under; have the watcher suppress warnings only for pipelines of requests whose scene is
//! still loading, each scene with its own STARTUP_FRAMES-style timeout and its own "precompile caught up
//! for this scene" check; and, for visible pop-in, hide a scene until its requests are built.
use bevy::prelude::*;
use corn_scene::{Ready, SceneFileRoot};

use crate::pipeline_watch::PipelineWatchArm;

/// CLAUDE: set once a scene file's root has finished spawning
#[derive(Resource, Default)]
struct SceneSpawned(bool);

// CLAUDE: arms pipeline_watch when the player, the scene and (web) the click have all happened.
pub fn plugin(app: &mut App) {
    app.init_resource::<SceneSpawned>()
        .add_observer(|ready: On<Ready>, roots: Query<(), With<SceneFileRoot>>, mut spawned: ResMut<SceneSpawned>| {
            if roots.contains(ready.entity) { spawned.0 = true; }
        })
        .add_systems(Update, arm.run_if(|arm: Option<Res<PipelineWatchArm>>| arm.is_some_and(|a| !a.armed())));
}

fn arm(
    watch: Res<PipelineWatchArm>, spawned: Res<SceneSpawned>,
    players: Query<(), With<crate::controller::Player>>,
) {
    if spawned.0 && !players.is_empty() && crate::browser::captured() {
        info!("pipeline_watch armed: scene and player spawned{}", if cfg!(target_arch = "wasm32") { ", pointer captured" } else { "" });
        watch.arm();
    }
}
