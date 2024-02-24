/*
    Main plugin for the game
    handles the state transition between loading, gameplay, and closing
*/

pub mod gameplay;
pub mod loading;
pub mod audio;

use std::time::Duration;

use bevy::{app::AppExit, prelude::*};
use bevy_rapier3d::plugin::{NoUserData, RapierPhysicsPlugin};
use loading::LoadGamePlugin;
use gameplay::CornGamePlayPlugin;

use self::audio::MyAudioPlugin;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum CornGameState{
    #[default]
    Init,
    Loading,
    Gameplay,
    Exit
}

#[derive(Default, Resource)]
pub struct LoadingTimer(Duration);

pub struct CornAppPlugin;
impl Plugin for CornAppPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
            .init_state::<CornGameState>()
            .init_resource::<LoadingTimer>()
            .add_systems(OnEnter(CornGameState::Init), init_game)
            .add_systems(OnExit(CornGameState::Loading), finish_loading)
            .add_systems(OnEnter(CornGameState::Exit), exit_game)
            .add_plugins((
                LoadGamePlugin::<CornGameState>::new(
                    CornGameState::Loading, 
                    CornGameState::Gameplay
                ),
                CornGamePlayPlugin::<CornGameState>::new(
                    CornGameState::Gameplay,
                    CornGameState::Exit
                ),
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

