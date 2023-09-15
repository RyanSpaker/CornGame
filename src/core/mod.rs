/*
    Main plugin for the game
    handles the state transition between loading, gameplay, and closing
*/

pub mod gameplay;
pub mod loading;

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

pub struct CornPlugin{}
impl Plugin for CornPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_state::<CornGameState>()
            .add_systems(OnEnter(CornGameState::Init), init_game)
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
    mut state: ResMut<NextState<CornGameState>>
){
    //Init work

    state.set(CornGameState::Loading);
}

pub fn exit_game(
    mut exit: EventWriter<AppExit>
){
    exit.send(AppExit{});
}

pub trait StatePlugin<T>: Plugin where T: States + Copy{

}
