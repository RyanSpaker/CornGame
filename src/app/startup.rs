use std::time::Duration;
use bevy::prelude::*;
use super::AppState;

/// Resource containing all tasks that need to be completed before finishing the Startup stage of the app
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct StartupTasks{
    /// Contains a list of strings representing the 
    pub task_ids: Vec<Entity>
}

/// System set for systems that run during the startup stage of the app
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct AppInitSystems;

pub enum AppInitSystemsEnabled{
    True,
    False
}

pub struct StartupPlugin;
impl Plugin for StartupPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_resource::<LoadingTimer>()
            .configure_sets(Main, AppInitSystems.run_if(in_state(AppState::Init)))
            .add_systems(Startup, loading_start)
            .add_systems(OnExit(AppState::Init), loading_end);

    }
}

#[derive(Default, Clone, Copy, Hash, PartialEq, Eq, Reflect, Resource)]
pub struct LoadingTimer(Duration);

pub fn loading_start(time: Res<Time>, mut loading_timer: ResMut<LoadingTimer>){
    //Init work
    info!("App Initialization Started");
    loading_timer.0 = time.elapsed();
}

pub fn loading_end(time: Res<Time>, loading_timer: ResMut<LoadingTimer>){
    info!("App Initialization Finished, Elapsed Millis: {}", (time.elapsed() - loading_timer.0).as_millis());
}