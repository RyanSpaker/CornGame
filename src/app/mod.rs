pub mod menu;
pub mod gameplay;
pub mod loading;
pub mod audio;

use std::time::Duration;
use bevy::{app::AppExit, prelude::*};
use self::{audio::MyAudioPlugin, menu::AppMenuState, gameplay::GameplayState};


/// Enum for representing the state of the application
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum AppState{
    /// Initial State of the app, responsible for loading bare essential assets, and preparing for a main menu scene
    #[default] Startup,
    /// State after loading, represents an app that is ready to be interacted with. Contains a Menu state and a Level state
    Open{
        /// State of the app menu. (main menu, settings, etc).
        menu_state: AppMenuState,
        /// State of the game. (Level, gameplay menus, etc).
        gameplay_state: GameplayState 
    }
}

#[derive(Default, Resource)]
pub struct LoadingTimer(Duration);

pub struct CornAppPlugin;
impl Plugin for CornAppPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(schedule, systems)
        app
            .init_state::<AppState>()
            .add_plugins((
                StartupPlugin,
                MyAudioPlugin
            ));
    }
}

pub fn init_game(
    mut state: ResMut<NextState<CornGameState>>,
    time: Res<Time>,
    mut loading_timer: ResMut<LoadingTimer>
){
    //Init work
    info!("Loading Start");
    loading_timer.0 = time.elapsed();
    state.set(CornGameState::Loading);
}

pub fn finish_loading(
    time: Res<Time>,
    loading_timer: ResMut<LoadingTimer>
){
    info!("Loading Finished, Elapsed Millis: {}", (time.elapsed() - loading_timer.0).as_millis());
}

pub fn exit_game(
    mut exit: EventWriter<AppExit>
){
    exit.send(AppExit{});
}

