/*
    Main plugin for the game
    handles the state transition between loading, gameplay, and closing
*/

pub mod gameplay;
pub mod loading;

use std::time::Duration;

use bevy::{prelude::*, app::AppExit};
use loading::LoadGamePlugin;
use gameplay::CornGamePlayPlugin;
use crate::{assets::CornAssetPlugin, ecs::CustomComponentRenderingPlugin};

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

pub struct CornPlugin{}
impl Plugin for CornPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_state::<CornGameState>()
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
                CornAssetPlugin{},
                CustomComponentRenderingPlugin{}
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

