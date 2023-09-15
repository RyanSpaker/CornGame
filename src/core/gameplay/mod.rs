use bevy::prelude::*;
use crate::flycam::{cam_look_plugin::CamLookPlugin, cam_move_plugin::CamMovePlugin};

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
            .insert_resource(GamePlayExitState(self.exit_state))
            .add_plugins((
                CamLookPlugin::<T>::new(self.active_state),
                CamMovePlugin::<T>::new(self.active_state)
            ))
            .add_systems(Update, exit_state_on_key::<T>.run_if(in_state(self.active_state)));
    }
}

fn exit_state_on_key<T: States + Copy>(
    input: Res<Input<KeyCode>>,
    exit_state: Res<GamePlayExitState::<T>>,
    mut next_state: ResMut<NextState<T>>
){
    if input.just_released(KeyCode::Escape){
        next_state.set(exit_state.0);
    }
}
