//! # Loading Module
//! The loading module defines a common loading architecture for the app. 
//! Loading is used to specify functionality and assets that must be run or made available during certain states of the app.
//! ## Overview:
//! - An LoadEntity is an entity that represents something that must be loaded. Examples include assets that must be created, systems that must be run, or state invariants that must be ensured.
//! - LoadEntities are components, together with [`LoadStatus`], and [`LoadForState`] components. 
//! [`LoadStatus`] components store that status of the load entity, including its unique name, and whether the entity is currently loaded.
//! [`LoadForState`] components are added for each state dependency this entity has, and contains all dependent states.
//! - LoadEntities depend on app states, meaning when the app enters a state, it will ensure the LoadEntities dependent on that state are loaded.
//! - A LoadEntity is added to the app by creating a struct that impl's LoadEntity, and calling app.insert_loader. State dependencies are added by calling app.register_state_req.
//! - Each [`LoadEntity`] can specify systems that run once when initiating loading, once per frame while loading, and once after loading. They also usually must specify a system that returns whether the LoadEntity is loaded.
//! - States that [`LoadEntity`]'s depend on have a LoadingState<S> added as a state to the app.
//! ## Design Principles:
//! - A major goal was to have the system have low overhead especially when there is no loading to be done. 
//! This was accomplished by placing systems into OnEnter schedules which only run when necessary, and using system sets to disable large amounts of systems at a time when necessary.
//! - Systems should run in a parallel fashion using schedules, rather than sequentially with mutable world access.
//! ## Scheduling:
//! Scheduling is roughly seperated into the following series of events
pub mod state;
pub mod entity;
pub mod quad;

use std::{any::type_name, collections::BTreeSet, marker::PhantomData, sync::{Arc, Mutex}};
use bevy::{ecs::system::{SystemParam, SystemParamItem}, prelude::*, utils::hashbrown::HashSet};
use state::*;

pub mod load_entity{
    use std::{any::type_name, sync::{atomic::AtomicBool, Arc}};
    use bevy::{ecs::{schedule::SystemConfigs, system::{SystemParam, SystemParamItem}}, prelude::*};

    use crate::app::AppStage;

    use super::GlobalLoadingState;
    
    /// Component storing the load status of the loader.
    #[derive(Debug, Default, Clone, Reflect, Component)]
    pub struct LoadStatus{
        pub load_state: LoadState,
        pub name: String,
        pub loaded_this_frame: Arc<AtomicBool>
    }
    #[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect)]
    pub enum LoadState{
        #[default] Unloaded,
        Loading,
        Loaded
    }

    /// Stores the state that this load entity depends on
    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
    pub struct StateDependency<S: States>(pub S);

    /// An object that needs to be loaded during specific states.
    pub trait LoadEntity: Component{
        type IsLoadedParams: SystemParam+Send+Sync+'static;
        type StateReq: States;
        /// Returns the unique name for this Load Dependency
        fn name(&self) -> String {type_name::<Self>().into()}
        /// Returns the state dependency of this loadentity
        fn state_req(&self) -> Self::StateReq;
        /// returns whether this loader is loaded or not
        fn is_loaded(&self, _entity: Entity) -> impl FnMut(SystemParamItem::<Self::IsLoadedParams>)->bool+Send+Sync+'static{
            move |_| {true}
        }
        /// Returns systems that run once when loading starts
        fn init_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
        /// Returns systems that run every frame while loading
        fn update_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
        /// Returns systems that run once when loading has finished
        fn cleanup_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
    }

    #[derive(Component)]
    pub struct LoadTest;
    impl LoadEntity for LoadTest{
        type StateReq = AppStage;
        type IsLoadedParams = (Res<'static, GlobalLoadingState>, Query<'static, 'static, &'static LoadStatus>);
        fn state_req(&self) -> Self::StateReq {AppStage::Level}
        fn is_loaded(&self, entity: Entity) -> impl FnMut(SystemParamItem::<Self::IsLoadedParams>)->bool+Send+Sync+'static {
            move |(state, query)| {
                if let GlobalLoadingState::Loaded = state.into_inner() {false}
                else if query.get(entity).is_ok_and(|status| status.load_state==LoadState::Loading) {true}
                else {false}
            }
        }
    }

    #[derive(Debug, Clone, Bundle)]
    pub struct LoadEntityBundle<L: LoadEntity>{
        status: LoadStatus,
        state: StateDependency<L::StateReq>
    }
    impl<L: LoadEntity> From<&L> for LoadEntityBundle<L> {
        fn from(value: &L) -> Self {
            let status = LoadStatus{name: value.name(), ..Default::default()};
            let state = StateDependency(value.state_req());
            Self{status, state}
        }
    }
}
use load_entity::*;


use super::state::state_set::{AppStateSets, ResourceSystemSet, StateSystemSet};
/*
    System Sets
*/
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub enum LoadEntitySet<S: States>{
    Init(S),
    Update(S),
    Cleanup(S),
    Check(S)
}

/// System which sets load entities that are currently loading to LoadState::Loading. Prevents repeated activation of init systems and allows update systems to run
pub fn move_to_loading<S: States>(state: Res<State<S>>, mut query: Query<&mut LoadStatus>, reqs: Res<PerStateLoadReqs<S>>){
    let Some(reqs) = reqs.get(state.get()) else {return;};
    for status in query.iter_mut().filter(|status| 
        status.load_state == LoadState::Unloaded && reqs.contains(&status.name)
    ){
        status.into_inner().load_state = LoadState::Loading;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct LoadEntityNotInitializedSet(pub Entity);
/// Returns a run condition that only runs the system when a specific entity has not yet run their init functions
pub fn load_entity_not_initialized(entity: Entity) -> impl FnMut(Query<&LoadStatus>)->bool{
    move |query: Query<&LoadStatus>| {
        let Ok(status) = query.get(entity) else {return false;};
        status.load_state==LoadState::Unloaded
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct LoadEntityIsLoadingSet(pub Entity);
/// Retruns a run condition that only runs the system when a specific entitty is loading.
pub fn load_entity_is_loading(entity: Entity) -> impl FnMut(Query<&LoadStatus>)->bool{
    move |query: Query<&LoadStatus>| {
        let Ok(status) = query.get(entity) else {return false;};
        status.load_state == LoadState::Loaded
    }
}

/// A resource holding all entities that have loaded this frame
#[derive(Default, Debug, Clone, Resource)]
pub struct EntitiesLoadedThisFrame(Arc<Mutex<HashSet<Entity>>>);
/// A system which if passed in true, will add the entity to the set of entities loaded this frame
pub fn notify_loaded(entity: Entity) -> impl FnMut(In<bool>, Res<EntitiesLoadedThisFrame>) {
    move |loaded: In<bool>, res: Res<EntitiesLoadedThisFrame>| {
        if !loaded.0 {return;}
        if let Ok(mut lock) = res.0.lock() {
            lock.insert(entity);
        }
    }
}

pub struct LoadEntityIsLoadedFunction<P, F> where
    P: SystemParam+Send+Sync+'static,
    F: FnMut(SystemParamItem<P>)->bool+Send+Sync+'static
{
    pub func: F,
    _p: PhantomData<P>
}
impl<P, F> SystemParamFunction<()> for LoadEntityIsLoadedFunction<P, F> where 
    P: SystemParam+Send+Sync+'static,
    F: FnMut(SystemParamItem<P>)->bool+Send+Sync+'static
{
    type In = ();
    type Out = bool;
    type Param = P;
    fn run(
        &mut self,
        _: <Self::In as SystemInput>::Inner<'_>,
        param_value: bevy::ecs::system::SystemParamItem<Self::Param>,
    ) -> Self::Out {
        (self.func)(param_value)
    }
}
impl<F, P> From<F> for LoadEntityIsLoadedFunction<P, F> where 
    P: SystemParam+Send+Sync+'static,
    F: FnMut(SystemParamItem<P>)->bool+Send+Sync+'static
{
    fn from(func: F) -> Self {
        Self{func, _p: PhantomData::default()}
    }
}


/*
    PLUGINS
*/
/// App trait to add loading system functionality
pub trait LoadingSystem{
    /// Inserts a Loading Dependency into the app
    fn insert_loader<D: LoadEntity>(&mut self, loader: D) -> &mut Self;
}
impl LoadingSystem for App{
    fn insert_loader<D: LoadEntity>(&mut self, loader: D) ->&mut App{
        self.add_plugins(LoadingPerStatePlugin::new(loader.state_req()));
        let state_reqs = self.world_mut().resource_mut::<PerStateLoadReqs<D::StateReq>>().into_inner();
        if let Some(reqs) = state_reqs.get_mut(&loader.state_req()) {
            reqs.insert(loader.name());
        } else {
            state_reqs.insert(loader.state_req(), BTreeSet::from([loader.name()]));
        }
        // Spawn entity
        let entity = self.world_mut().spawn(LoadEntityBundle::from(&loader)).id();
        // Schedule loader systems
        if let Some(init) = loader.init_systems(entity){
            self.add_systems(OnEnter(LoadingPerState::Loading(loader.state_req())), 
                init.in_set(LoadEntityNotInitializedSet(entity)).in_set(LoadEntitySet::Init(loader.state_req()))
            );
            self.configure_sets(OnEnter(LoadingPerState::Loading(loader.state_req())), 
                LoadEntityNotInitializedSet(entity).run_if(load_entity_not_initialized(entity))
            );
        }
        if let Some(update) = loader.update_systems(entity){
            self.add_systems(Update, 
                update.in_set(LoadEntityIsLoadingSet(entity)).in_set(LoadEntitySet::Update(loader.state_req()))
            );
        }
        self.add_systems(Update, 
            LoadEntityIsLoadedFunction::from(loader.is_loaded(entity)).pipe(notify_loaded(entity)).in_set(LoadEntitySet::Check(loader.state_req()))
        );
        self.configure_sets(Update, 
            LoadEntityIsLoadingSet(entity).run_if(load_entity_is_loading(entity))
        );
        self.world_mut().entity_mut(entity).insert(loader);
        self
    }
}
/// Global loading system plugin
#[derive(Default, Debug, Clone)]
pub struct LoadingPlugin;
impl Plugin for LoadingPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_plugins(GlobalLoadingState::default())
            .init_state::<LoadedEntities>()
            .add_systems(PreUpdate, LoadedEntities::update);
    }
}
/// Per state type loading system plugin
#[derive(Debug, Clone)]
pub struct LoadingStateTypePlugin<S: States>(PhantomData<S>);
impl<S: States> Default for LoadingStateTypePlugin<S>{fn default() -> Self {Self(PhantomData::default())}}
impl<S: States> Plugin for LoadingStateTypePlugin<S>{
    fn build(&self, app: &mut App) {
        app
            .add_plugins(LoadingPlugin)
            .init_resource::<PerStateLoadReqs<S>>()
            .add_computed_state::<LoadingPerState<S>>()
            .add_computed_state::<LoadingState<S>>()
            .configure_state_set(LoadingState::<S>::Loading, Update, Some(ResourceSystemSet(GlobalLoadingState::Loading)))
            .configure_state_set(LoadingState::<S>::Loaded, Update, Some(ResourceSystemSet(GlobalLoadingState::Loaded)));
    }
    fn finish(&self, app: &mut App) {
        // Switches the perstateloadreqs resource into a state. it is done this way so that we can build up the state at runtime, before setting it as a state, at which point it becomes basically immutable.
        if let Some(res) = app.world_mut().remove_resource::<PerStateLoadReqs<S>>() {
            app.insert_state(res);
        }
    }
}
/// Per state instance loading system plugin
#[derive(Default, Debug, Clone)]
pub struct LoadingPerStatePlugin<S: States>{
    state: S, 
    name: String
}
impl<S: States> LoadingPerStatePlugin<S>{
    pub fn new(state: S) -> Self{Self{
        name: type_name::<Self>().to_string() + format!("{:?}", state).as_str(), 
        state
    }}
}
impl<S: States> Plugin for LoadingPerStatePlugin<S>{
    fn build(&self, app: &mut App) {
        let state = self.state.clone();
        app
            .add_plugins(LoadingStateTypePlugin::<S>::default())
            .add_systems(OnEnter(state.clone()), LoadingTargetCount::get_increment_system())
            .add_systems(OnExit(state.clone()), LoadingTargetCount::get_decrement_system())
            .configure_state_set(LoadingPerState::Loading(state.clone()), Update, Some(ResourceSystemSet(GlobalLoadingState::Loading)))
            .configure_state_set(LoadingPerState::Loaded(state.clone()), Update, Some(ResourceSystemSet(GlobalLoadingState::Loaded)))
            .configure_sets(Update, LoadEntitySet::Update(state.clone()).in_set(StateSystemSet(LoadingPerState::Loading(state.clone()))))
            .add_systems(OnEnter(LoadingPerState::Loading(state.clone())), move_to_loading::<S>.after(LoadEntitySet::Init(state.clone())));
    }
    fn name(&self) -> &str {&self.name}
}


/*
    OnEnter(State): Look to see if dependencies are still loading or unloaded. If so switch LoadingState->Loading else send LoadingState->Loaded.
    apply_state_transisition::<LoadingState>
    OnEnter(LoadingState::Loading(State)): init_systems: run_if loadingstate::unloaded. After, switch all loadingstate::unloaded->loading
    Update: 
        GlobalLoadingSet
            LoadingSet::Loading(State),
                chain: (UpdateSystems, CheckSystems), always run, per: run_if loadstate::loading, run_if(none of prev added state reqs are loading)
        CleanupSet: run_if on_event CleanupEvent. per: run_if waiting_for_cleanup. then: set all waiting_for_cleanup: false
    End Update: Observers are run. When an observer finds that loading has finished, It sets loadingstate->loaded, enables waiting_for_c;eanup bool, and sends  a cleanup event
    LateUpdate:
        LoadingSet::Loading(State)
            Check_loading: check if any load entities are still loading, if not LoadingState->Loaded
*/

