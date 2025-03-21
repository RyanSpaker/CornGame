use bevy::{prelude::*, utils::hashbrown::{HashMap, HashSet}};
use std::{hash::{Hash, Hasher}, marker::PhantomData};
use super::entity::{LoadEntityStatus, LoadStage, StateDependency};

/*
    Manually Calculated States
*/

/// State struct calculated each frame holding the state of each load entity
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, States)]
pub struct LoadEntityGlobalState{
    pub loading_entities: HashMap<Entity, LoadStage>,
    pub loaded_entities: HashSet<Entity>,
    pub unloading_entities: HashMap<Entity, LoadStage>,
    pub unloaded_entities: HashSet<Entity>
}
impl std::hash::Hash for LoadEntityGlobalState{fn hash<H: Hasher>(&self, state: &mut H) {
    for i in self.loading_entities.iter() {i.0.hash(state); i.1.hash(state);}
    for i in self.loaded_entities.iter() {(*i).hash(state);}
    for i in self.unloading_entities.iter() {i.0.hash(state); i.1.hash(state);}
    for i in self.unloaded_entities.iter() {(*i).hash(state);}
}}
impl LoadEntityGlobalState{
    pub fn calculate(
        query: Query<(Entity, &LoadEntityStatus)>,
        state: Res<State<Self>>,
        mut next_state: ResMut<NextState<Self>>
    ){
        let mut new_state = Self::default();
        for (entity, state) in query.into_iter(){
            match state{
                LoadEntityStatus::Unloaded => {new_state.unloaded_entities.insert(entity);},
                LoadEntityStatus::Loading(stage) => {new_state.loading_entities.insert(entity, stage.to_owned());},
                LoadEntityStatus::Loaded => {new_state.loaded_entities.insert(entity);},
                LoadEntityStatus::Unloading(stage) => {new_state.unloading_entities.insert(entity, stage.to_owned());},
            };
        }
        if *state.get() != new_state {
            next_state.set(new_state);
        }
    }
}

/*
    Computed States
*/

/// State which partitions states into sets of loading, loaded, unloading, and unloaded
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct StateLoadingStates<S: States>{
    pub loading: HashSet<S>,
    pub loaded: HashSet<S>,
    pub unloading: HashSet<S>,
    pub unloaded: HashSet<S>
}
impl<S: States> Default for StateLoadingStates<S>{fn default() -> Self {Self { 
    loading: HashSet::default(), loaded: HashSet::default(), unloading: HashSet::default(), unloaded: HashSet::default() 
}}}
impl<S: States> std::hash::Hash for StateLoadingStates<S>{fn hash<H: Hasher>(&self, state: &mut H) {
    std::any::TypeId::of::<S>().hash(state);
    for s in self.loading.iter() {(*s).hash(state);}
    for s in self.loaded.iter() {(*s).hash(state);}
    for s in self.unloading.iter() {(*s).hash(state);}
    for s in self.unloaded.iter() {(*s).hash(state);}
}}
impl<S: States> ComputedStates for StateLoadingStates<S>{
    type SourceStates = (LoadEntityGlobalState, StateLoadDependencies<S>);
    fn compute(sources: Self::SourceStates) -> Option<Self> {
        let mut computed = Self::default();
        for (state, set) in sources.1.reqs.iter() {
            if sources.0.loaded_entities.is_superset(set) {computed.loaded.insert(state.to_owned());}
            if sources.0.unloaded_entities.is_superset(set) {computed.unloaded.insert(state.to_owned());}
            if !sources.0.loading_entities.is_disjoint(set) {computed.loading.insert(state.to_owned());}
            if !sources.0.unloading_entities.is_disjoint(set) {computed.unloading.insert(state.to_owned());}
        }
        Some(computed)
    }
}

/// State which says whether the current state S has all of its deps loaded or not
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub enum LoadingState<S: States>{
    #[default] Unloaded,
    Loaded,
    Err(PhantomData<S>)
}
impl<S: States> ComputedStates for LoadingState<S>{
    type SourceStates = (S, StateLoadDependencies<S>, LoadEntityGlobalState);
    fn compute(sources: Self::SourceStates) -> Option<Self> {
        if let Some(deps) = sources.1.reqs.get(&sources.0) {
            if sources.2.loaded_entities.is_superset(deps) {Some(Self::Loaded)}
            else {Some(Self::Unloaded)}
        }else {Some(Self::Loaded)}
    }
}

/*
    System Sets
*/

/// Set that runs when anything is loading
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct GlobalLoadingSet;
impl GlobalLoadingSet{
    pub fn run_condition(state: Res<State<LoadEntityGlobalState>>) -> bool{
        !state.get().loading_entities.is_empty()
    }
}

/// Set that runs when a specific entity is loading
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct EntityLoadingSet(pub Entity, pub LoadStage);
impl EntityLoadingSet{
    pub fn run_condition(&self) -> impl FnMut(Res<State<LoadEntityGlobalState>>)->bool{
        let entity = self.0.to_owned();
        let stage = self.1.to_owned();
        move |state: Res<State<LoadEntityGlobalState>>| {
            state.loading_entities.get(&entity) == Some(&stage)
        }
    }
}

/// Set that runs when a specific state is loading
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct StateLoadingSet<S: States>(pub S);
impl<S: States> StateLoadingSet<S>{
    pub fn run_condition(&self) -> impl FnMut(Query<(&IsLoading, &StateDependency<S>)>)->bool{
        let state = self.0.to_owned();
        move |query: Query<(&IsLoading, &StateDependency<S>)>| {
            query.iter().any(|(_, deps)| deps.0.contains(&state))
        }
    }
}

/*
    Plugins and App Trait
*/

/// Adds functionality to add a state load req anywhere in the app.
pub trait AppLoadStateSystem{
    /// Setup app structure for handling a state with LoadEntities that depend on it.
    fn register_state_req<S: States>(&mut self, state: S);
}
impl AppLoadStateSystem for App{
    fn register_state_req<S: States>(&mut self, state: S) {
        self
            .add_plugins(PerStateLoadingPlugin::<S>::default())
            .configure_sets(Update, StateLoadingSet(state.clone())
                .run_if(StateLoadingSet(state.clone()).run_condition())
                .in_set(GlobalLoadingSet)
            );
    }
}

/// Plugin adding per state type loading state functionality to the app.
#[derive(Debug, Clone)]
pub struct PerStateLoadingPlugin<S: States>(PhantomData<S>);
impl<S: States> Default for PerStateLoadingPlugin<S>{fn default() -> Self {Self(PhantomData::default())}}
impl<S: States> Plugin for PerStateLoadingPlugin<S>{
    fn build(&self, app: &mut App) {
        app
            .add_plugins(LoadStatePlugin)
            .init_state::<StateLoadDependencies<S>>()
            .add_systems(PreUpdate, StateLoadDependencies::<S>::compute)
            .add_computed_state::<StateLoadingStates<S>>()
            .add_computed_state::<LoadingState<S>>();
    }
}

/// Plugin adding universal loading state functionality
#[derive(Default, Debug, Clone)]
pub struct LoadStatePlugin;
impl Plugin for LoadStatePlugin{
    fn build(&self, app: &mut App) {
        app
            // LoadedEntities
            .init_state::<LoadEntityGlobalState>()
            .add_systems(PreUpdate, LoadEntityGlobalState::calculate)
            // GlobalLoadingSet
            .configure_sets(Update, GlobalLoadingSet.run_if(GlobalLoadingSet::run_condition));
    }
}

