pub mod entity;

use std::marker::PhantomData;
use bevy::prelude::*;
use entity::{GlobalLoadEntityState, LoadEntityStatus, StateDependency};

/// Function which loads any requirements of the state that are not loaded
pub fn auto_load_requirements<S: States>(
    state: Res<State<S>>, 
    mut query: Query<(&mut LoadEntityStatus, &StateDependency<S>)>
) {
    for (status, deps) in query.iter_mut(){
        let status = status.into_inner();
        if let LoadEntityStatus::Unloaded = status{
            if deps.0 == *state.get() {
                *status = LoadEntityStatus::Loading(entity::LoadStage::Init);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutoLoadPlugin<S: States>(PhantomData<S>);
impl<S: States> Default for AutoLoadPlugin<S> {fn default() -> Self {Self(PhantomData::default())}}
impl<S: States> Plugin for AutoLoadPlugin<S>{
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, auto_load_requirements::<S>.before(GlobalLoadEntityState::update));
    }
}