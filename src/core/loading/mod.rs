/*
    Handles the Loading of the game,
    This includes the reading of the corn asset file
    it also includes the initial scene setup
*/
pub mod setup_scene_plugin;

use bevy::prelude::*;
use crate::assets::corn_model::LoadCornPlugin;
use setup_scene_plugin::SetupScenePlugin;

#[derive(Resource, Default)]
pub struct LoadingTaskCount(pub usize);

#[derive(Resource, Default)]
pub struct LoadingExitState<T>(T) where T: States + Copy;

pub struct LoadGamePlugin<T> where T: States + Copy{
    active_state: T,
    exit_state: T
}
impl<T> LoadGamePlugin<T> where T: States + Copy{
    pub fn new(active_state: T, exit_state: T) -> Self {
        Self {active_state, exit_state}
    }
}
impl<T> Plugin for LoadGamePlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .insert_resource(LoadingTaskCount(0))
            .insert_resource(LoadingExitState::<T>(self.exit_state))
            .add_systems(
                Update, 
                (schedule_exit_loading_state::<T>).run_if(in_state(self.active_state))
            ).add_plugins((
                LoadCornPlugin::<T>::new(self.active_state),
                SetupScenePlugin::<T>::new(self.active_state)
            ));
    }
}

fn schedule_exit_loading_state<T>(
    task_count: Res<LoadingTaskCount>,
    mut next_state: ResMut<NextState<T>>,
    exit_state: Res<LoadingExitState<T>>
) where T: States + Copy{
    if task_count.0 == 0{
        next_state.set(exit_state.0);
    }
}