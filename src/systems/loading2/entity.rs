//! Goals:
//! - Easily specify load requirements anywhere in the code, without any central refactors or changes.
//! - Specify states that a load requirement is needed by
//! - Be able to have a central system for scheduling common function types for loading requirements such as init, update, cleanup systems
//! - Lean into the bevy architecture with loading systems design.
//! - Low overhead
//! - Easy way to create blocking load states that wait for loading requirements before entering the state.
//! - 
//! 
//! - Each LoadEntity is an Entity.
//! - Load Entities can be registered in the app, which auto schedules systems sets that only run when the LoadEntity is loading
//! 
use std::{collections::{BTreeMap, BTreeSet}, marker::PhantomData};
use bevy::{ecs::{schedule::{ScheduleLabel, SystemConfigs}, system::{SystemParam, SystemParamItem}}, prelude::*, utils::hashbrown::{HashMap, HashSet}};

/*
    Resources
*/

/// Resource which calculates info about all load entities every frame. Uses Changed<T> to only update when necessary
#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct GlobalLoadEntityState{
    loading: BTreeSet<Entity>,
    unloading: BTreeSet<Entity>,
    stages: BTreeMap<Entity, LoadStage>,
    names: BTreeMap<String, Entity>
}
impl GlobalLoadEntityState{
    pub fn update(
        query: Query<(Entity, &LoadEntityName, &LoadEntityStatus), Or<(Changed<LoadEntityStatus>, Changed<LoadEntityName>)>>,
        mut res: ResMut<Self>
    ){
        for (entity, name, status) in query.iter(){
            res.names.insert(name.0.to_owned(), entity);
            match status{
                LoadEntityStatus::Loaded => {
                    res.loading.remove(&entity); res.unloading.remove(&entity); res.stages.remove(&entity);
                }
                LoadEntityStatus::Unloaded => {
                    res.loading.remove(&entity); res.unloading.remove(&entity); res.stages.remove(&entity);
                }
                LoadEntityStatus::Loading(stage) => {
                    res.unloading.remove(&entity); res.loading.insert(entity); res.stages.insert(entity, stage.to_owned());
                }
                LoadEntityStatus::Unloading(stage) => {
                    res.loading.remove(&entity); res.unloading.insert(entity); res.stages.insert(entity, stage.to_owned());
                }
            }
        }
    }
    /// Remove deleted LoadEntities. Any LoadEntity where the name is removed is considered deleted as all load entities need a status and name
    pub fn remove(
        mut removed: RemovedComponents<LoadEntityName>,
        mut res: ResMut<Self>
    ){
        let old: HashSet<Entity> = removed.read().collect();
        res.loading.retain(|k| !old.contains(k));
        res.unloading.retain(|k| !old.contains(k));
        res.stages.retain(|k, _| !old.contains(k));
        res.names.retain(|_, v| !old.contains(v));
    }
}

/// Resource which calculates info about all load entities necessary for instances of a state type. Uses Changed<T> to only update when necessary
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct StateLoadEntityReqs<S: States>{
    dependencies: BTreeMap<Entity, S>,
    state_reqs: HashMap<S, BTreeSet<Entity>>
}
impl<S: States> Default for StateLoadEntityReqs<S>{fn default() -> Self {Self{dependencies: BTreeMap::default(), state_reqs: HashMap::default()}}}
impl<S: States> StateLoadEntityReqs<S>{
    fn update(
        query: Query<(Entity, &StateDependency<S>), Changed<StateDependency<S>>>,
        mut res: ResMut<Self>
    ){
        for (entity, dep) in query.iter(){
            res.dependencies.insert(entity, dep.0.to_owned());
            if let Some(set) = res.state_reqs.get_mut(&dep.0){
                set.insert(entity);
            }else {res.state_reqs.insert(dep.0.to_owned(), BTreeSet::from([entity]));}
        }
    }
    fn remove(
        mut removed: RemovedComponents<StateDependency<S>>,
        mut res: ResMut<Self>
    ){
        for entity in removed.read(){
            if let Some(state) = res.dependencies.remove(&entity){
                if let Some(set) = res.state_reqs.get_mut(&state){
                    set.remove(&entity);
                }
            }
        }
    }
}

/*
    System Sets
*/

/// System Set that runs when there is any entity loading
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub enum GlobalLoadingSet{
    Loading,
    Unloading
}
impl GlobalLoadingSet{
    pub fn condition(&self) -> impl FnMut(Res<GlobalLoadEntityState>)->bool{
        match self {
            Self::Loading => move |res: Res<GlobalLoadEntityState>| {
                !res.loading.is_empty()
            },
            Self::Unloading => move |res: Res<GlobalLoadEntityState>| {
                !res.unloading.is_empty()
            }
        }
    }
}

/// System Set that runs when at least one load req of the state is loading
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub enum StateLoadingSet<S: States>{
    Loading(S),
    Unloading(S)
}
impl<S: States> StateLoadingSet<S>{
    pub fn condition(&self) -> impl FnMut(Res<GlobalLoadEntityState>, Res<StateLoadEntityReqs<S>>) -> bool{
        let set = self.clone();
        move |res: Res<GlobalLoadEntityState>, reqs: Res<StateLoadEntityReqs<S>>| {
            match &set {
                Self::Loading(state) => {
                    if let Some(deps) = reqs.state_reqs.get(state) {
                        !res.loading.is_disjoint(deps)
                    } else {false}
                },
                Self::Unloading(state) => {
                    if let Some(deps) = reqs.state_reqs.get(state) {
                        !res.unloading.is_disjoint(deps)
                    } else {false}
                }
            }
        }
    }
}

/// System Set that runs when an entity is loading/unloading with loadstage
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub enum EntityLoadingSet{
    Loading(Entity, LoadStage),
    Unloading(Entity, LoadStage)
}
impl EntityLoadingSet{
    pub fn condition(&self)-> impl FnMut(Res<GlobalLoadEntityState>) -> bool{
        let set = self.clone();
        move |res: Res<GlobalLoadEntityState>| {
            match &set {
                Self::Loading(entity, stage) => {
                    res.loading.contains(entity) && res.stages.get(entity).is_some_and(|val| *val == *stage)
                },
                Self::Unloading(entity, stage) => {
                    res.unloading.contains(entity) && res.stages.get(entity).is_some_and(|val| *val == *stage)
                }
            }
        }
    }
}

/*
    Components
*/

/// Unique Name for each LoadEntity, used to make certain operations based on a human readale name rather than entity tag.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct LoadEntityName(pub String);

/// State of the LoadEntity
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub enum LoadEntityStatus{
    #[default] Unloaded,
    Loading(LoadStage),
    Loaded,
    Unloading(LoadStage)
}

/// Specific Stage of Loading/Unloading for a LoadEntity
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub enum LoadStage{
    #[default] Init,
    Update,
    Cleanup
}

/// Stores the state that this load entity depends on
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct StateDependency<S: States>(pub S);

/// An object that needs to be loaded during specific states.
pub trait LoadEntity: Component{
    type IsLoadedParams: SystemParam+Send+Sync+'static;
    type StateReq: States;
    /// Returns the state dependency of this loadentity
    fn state_req(&self) -> Self::StateReq;
    /// system run while loading to determine if loading is finished
    fn is_loaded(&self, _entity: Entity) -> impl FnMut(SystemParamItem::<Self::IsLoadedParams>)->bool+Send+Sync+'static {
        move |_| {true}
    }
    /// system run while loading to determine if loading is finished
    fn is_unloaded(&self, _entity: Entity) -> impl FnMut(SystemParamItem::<Self::IsLoadedParams>)->bool+Send+Sync+'static {
        move |_| {true}
    }
    /// Returns systems that run once when loading starts
    fn loading_init_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run every frame while loading
    fn loading_update_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run once when loading has finished
    fn loading_cleanup_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run once when unloading starts
    fn unloading_init_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run every frame while unloading
    fn unloading_update_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run once when unloading has finished
    fn unloading_cleanup_systems(&self, _entity: Entity) -> Option<SystemConfigs> {None}
}

/*
    Systems
*/

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

/// Event sent when LoadEntities finish Loading or Unloading
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, Event)]
pub enum LoadEntityFinishedEvent{
    Loaded(Entity),
    Unloaded(Entity)
}

/// Updates the load state of the LoadEntities
pub fn update_load_entity_state(
    mut query: Query<&mut LoadEntityStatus>,
    mut events: EventReader<LoadEntityFinishedEvent>
){
    for status in query.iter_mut(){
        let status = status.into_inner();
        match status{
            LoadEntityStatus::Loading(LoadStage::Init) => {*status = LoadEntityStatus::Loading(LoadStage::Update);}
            LoadEntityStatus::Loading(LoadStage::Cleanup) => {*status = LoadEntityStatus::Loaded;}
            LoadEntityStatus::Unloading(LoadStage::Init) => {*status = LoadEntityStatus::Unloading(LoadStage::Update);}
            LoadEntityStatus::Unloading(LoadStage::Cleanup) => {*status = LoadEntityStatus::Unloaded;}
            _ => {}
        }
    }
    for event in events.read(){
        match event {
            LoadEntityFinishedEvent::Loaded(entity) => {
                if let Ok(mut status) = query.get_mut(*entity){
                    *status = LoadEntityStatus::Loaded
                }
            }
            LoadEntityFinishedEvent::Unloaded(entity) => {
                if let Ok(mut status) = query.get_mut(*entity){
                    *status = LoadEntityStatus::Unloaded
                }
            }
        }
    }
}

/// System which sends a finished::loaded event for the entity
pub fn send_loaded_event(entity: Entity) -> impl FnMut(In<bool>, EventWriter<LoadEntityFinishedEvent>) {
    move |In(success): In<bool>, mut sender: EventWriter<LoadEntityFinishedEvent>| {
        if success {
            sender.send(LoadEntityFinishedEvent::Loaded(entity));
        }
    }
}
/// System which sends a finished::unloaded event for the entity
pub fn send_unloaded_event(entity: Entity) -> impl FnMut(In<bool>, EventWriter<LoadEntityFinishedEvent>) {
    move |In(success): In<bool>, mut sender: EventWriter<LoadEntityFinishedEvent>| {
        if success {
            sender.send(LoadEntityFinishedEvent::Unloaded(entity));
        }
    }
}

/*
    Plugins
*/

pub fn configure_loading_systems(app: &mut App, systems: Option<SystemConfigs>, schedule: impl ScheduleLabel+Clone, stage: LoadStage, state: impl States, entity: Entity){
    let set = EntityLoadingSet::Loading(entity, stage.to_owned());
    if let Some(systems) = systems {
        app.add_systems(schedule.clone(), systems.in_set(set.clone()))
        .configure_sets(schedule, set.in_set(StateLoadingSet::Loading(state)));
    }
}
pub fn configure_unloading_systems(app: &mut App, systems: Option<SystemConfigs>, schedule: impl ScheduleLabel+Clone, stage: LoadStage, state: impl States, entity: Entity){
    let set = EntityLoadingSet::Unloading(entity, stage.to_owned());
    if let Some(systems) = systems {
        app.add_systems(schedule.clone(), systems.in_set(set.clone()))
        .configure_sets(schedule, set.in_set(StateLoadingSet::Unloading(state)));
    }
}


/// App trait to add loading system functionality
pub trait LoadingSystem{
    /// Inserts a Loading Dependency into the app
    fn insert_loader<D: LoadEntity>(&mut self, loader: D) -> &mut Self;
}
impl LoadingSystem for App{
    fn insert_loader<D: LoadEntity>(&mut self, loader: D) ->&mut App{
        // Setup Plugins
        self.add_plugins(LoadingPerStatePlugin::new(loader.state_req()));
        // Spawn Entity
        let entity = self.world_mut().spawn(LoadEntityStatus::Unloaded).id();
        // Schedule loader systems
        configure_loading_systems(self, loader.loading_init_systems(entity), Update, LoadStage::Init, loader.state_req(), entity);
        configure_loading_systems(self, loader.loading_cleanup_systems(entity), Update, LoadStage::Cleanup, loader.state_req(), entity);
        configure_unloading_systems(self, loader.unloading_init_systems(entity), Update, LoadStage::Init, loader.state_req(), entity);
        configure_unloading_systems(self, loader.unloading_cleanup_systems(entity), Update, LoadStage::Cleanup, loader.state_req(), entity);
        // Add state management systems
        if let Some(systems) = loader.loading_update_systems(entity) {
            let set = EntityLoadingSet::Loading(entity, LoadStage::Update);
            self.add_systems(Update, 
                (systems, LoadEntityIsLoadedFunction::from(loader.is_loaded(entity)).pipe(send_loaded_event(entity))).in_set(set.clone())
            ).configure_sets(Update, set.in_set(StateLoadingSet::Loading(loader.state_req())));
        }else {
            let set = EntityLoadingSet::Loading(entity, LoadStage::Update);
            self.add_systems(Update, 
                LoadEntityIsLoadedFunction::from(loader.is_loaded(entity)).pipe(send_loaded_event(entity)).in_set(set.clone())
            ).configure_sets(Update, set.in_set(StateLoadingSet::Loading(loader.state_req())));
        }
        if let Some(systems) = loader.unloading_update_systems(entity) {
            let set = EntityLoadingSet::Unloading(entity, LoadStage::Update);
            self.add_systems(Update, 
                (systems, LoadEntityIsLoadedFunction::from(loader.is_unloaded(entity)).pipe(send_unloaded_event(entity))).in_set(set.clone())
            ).configure_sets(Update, set.in_set(StateLoadingSet::Unloading(loader.state_req())));
        }else {
            let set = EntityLoadingSet::Unloading(entity, LoadStage::Update);
            self.add_systems(Update, 
                LoadEntityIsLoadedFunction::from(loader.is_unloaded(entity)).pipe(send_unloaded_event(entity)).in_set(set.clone())
            ).configure_sets(Update, set.in_set(StateLoadingSet::Unloading(loader.state_req())));
        }
        // Add LoadEntity Component
        self.world_mut().entity_mut(entity).insert(loader);
        self
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
        name: std::any::type_name::<Self>().to_string() + format!("{:?}", state).as_str(), 
        state
    }}
}
impl<S: States> Plugin for LoadingPerStatePlugin<S>{
    fn build(&self, app: &mut App) {
        let state = self.state.clone();
        app
            .add_plugins(LoadingStateTypePlugin::<S>::default())
            .configure_sets(Update, StateLoadingSet::Loading(state.clone()).in_set(GlobalLoadingSet::Loading))
            .configure_sets(Update, StateLoadingSet::Unloading(state.clone()).in_set(GlobalLoadingSet::Unloading));
    }
    fn name(&self) -> &str {&self.name}
}

/// Per state type loading system plugin
#[derive(Debug, Clone)]
pub struct LoadingStateTypePlugin<S: States>(PhantomData<S>);
impl<S: States> Default for LoadingStateTypePlugin<S>{fn default() -> Self {Self(PhantomData::default())}}
impl<S: States> Plugin for LoadingStateTypePlugin<S>{
    fn build(&self, app: &mut App) {
        app
            .add_plugins((LoadingPlugin, super::AutoLoadPlugin::<S>::default()))
            .init_resource::<StateLoadEntityReqs<S>>()
            .add_systems(PreUpdate, StateLoadEntityReqs::<S>::update)
            .add_systems(PostUpdate, StateLoadEntityReqs::<S>::remove);
    }
}

/// Global loading system plugin
#[derive(Default, Debug, Clone)]
pub struct LoadingPlugin;
impl Plugin for LoadingPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_resource::<GlobalLoadEntityState>()
            .add_event::<LoadEntityFinishedEvent>()
            .add_systems(PreUpdate, GlobalLoadEntityState::update)
            .add_systems(PostUpdate, (GlobalLoadEntityState::remove, update_load_entity_state));
    }
}

