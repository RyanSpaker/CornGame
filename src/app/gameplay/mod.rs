use bevy::prelude::*;

pub mod character_controller;
pub mod npc;

use crate::ecs::flycam::{enable_flycam, FlyCamPlugin, FlyCamState};


#[derive(Resource, Default)]
pub struct GamePlayExitState<T>(T) where T: States + Copy;

pub struct CornGamePlayPlugin<T> where T: States + Copy{
    active_state: T,
    exit_state: T
}
impl<T> CornGamePlayPlugin<T> where T: States + Copy{
    pub fn new(active_state: T, exit_state: T) -> Self {
        Self {active_state, exit_state}
    }
}
impl<T> Plugin for CornGamePlayPlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                FlyCamPlugin::new(FlyCamState::Disabled),
                character_controller::CharacterControllerPlugin,
            ))
            .insert_resource(GamePlayExitState(self.exit_state))
            .add_systems(OnEnter(self.active_state), enable_flycam)
            .add_systems(Update, (
                exit_state_on_key::<T>
            ).run_if(in_state(self.active_state)));
    }
}

fn exit_state_on_key<T: States + Copy>(
    input: Res<ButtonInput<KeyCode>>,
    exit_state: Res<GamePlayExitState::<T>>,
    mut next_state: ResMut<NextState<T>>
){
    if input.just_released(KeyCode::Escape){
        next_state.set(exit_state.0);
    }
}

