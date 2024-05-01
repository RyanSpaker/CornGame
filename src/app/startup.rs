use bevy::{prelude::*, utils::hashbrown::HashSet};

use crate::ecs::loading::LevelDependency;

use super::AppState;

/// Resource containing all tasks that need to be completed before finishing the Startup stage of the app
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct StartupTasks{
    /// Contains a list of strings representing the 
    pub task_ids: Vec<Entity>
}
pub fn test(tasks: Res<StartupTasks>, query: Query<&LevelDependency, With<LevelDependency>>) {
    for entity in query.iter_many(&tasks.task_ids){
        
    }
}
/// System set for systems that run during the startup stage of the app
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct StartupSystems;

pub struct StartupPlugin;
impl Plugin for StartupPlugin{
    fn build(&self, app: &mut App) {
        app
            .configure_sets(Main, StartupSystems.run_if(in_state(AppState::Startup)));

    }
}