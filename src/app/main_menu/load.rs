use std::marker::PhantomData;
use bevy::{ecs::{schedule::SystemConfigs, system::SystemParam}, prelude::*, utils::hashbrown::HashSet};

use crate::{app::state::state_set::{AppStateSets, StateSystemSet}, ecs::corn::asset::{CornAsset, CornModel}};

/// Component storing the load status of the loader.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct LoaderStatus{
    pub loaded: bool,
    pub name: String
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct IsLoading;

/// Component used to store the states that require the Loader to be loaded
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct StateLoadReq<S: States>(HashSet<S>);

/// Enum representing the load state of other states in the app. Added for any state that has a Loader dependent on it.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum LoadingState<S: States>{
    #[default] Unloaded,
    Loading,
    Loaded,
    Err(PhantomData<S>)
}
/// Enum representing the global state of loading. loading contains a count of how many unique load targets are currently loading
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Resource)]
pub struct LoadingTargetCount(usize);
impl LoadingTargetCount{
    fn increment(mut res: ResMut<Self>) {
        res.0 += 1;
    }
    fn decrement(mut res: ResMut<Self>) {
        res.0 -= 1;
    }
    fn is_loading(res: Res<Self>) -> bool { 
        res.0>0
    }
}

/// An object that needs to be loaded during specific states.
pub trait LoaderEntity: Component{
    type IsLoadedParams<'w>: SystemParam;
    /// Returns the unique name for this Load Dependency
    fn name(&self) -> String;
    /// returns whether this loader is loaded or not
    fn is_loaded<'w>(&self, params: Self::IsLoadedParams<'w>) -> bool {true}
    /// Returns systems that run once when loading starts
    fn load_init_systems(&self, entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run every frame while loading
    fn load_update_systems(&self, entity: Entity) -> Option<SystemConfigs> {None}
    /// Returns systems that run once when loading has finished
    fn load_cleanup_systems(&self, entity: Entity) -> Option<SystemConfigs> {None}
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct CornModelLoadReq(Option<Handle<CornAsset>>);
impl CornModelLoadReq{
    pub fn spawn_corn_asset(entity: In<Entity>, mut corn_res: ResMut<CornModel>, assets: Res<AssetServer>, mut query: Query<&mut Self>) {
        corn_res.asset = assets.load("models/Corn.gltf");
        if let Ok(mut load_req) = query.get_mut(entity.0) {load_req.0 = Some(corn_res.asset.clone());}
    }
}
impl LoaderEntity for CornModelLoadReq{
    type IsLoadedParams<'w> = Res<'w, AssetServer>;
    fn name(&self) -> String {"Corn Model".to_string()}
    fn is_loaded<'w>(&self, params: Self::IsLoadedParams<'w>) -> bool {
        self.0.clone().is_some_and(|handle| params.is_loaded_with_dependencies(handle))
    }
    fn load_init_systems(&self, entity: Entity) -> Option<SystemConfigs> {
        Some(pass_entity(entity).pipe(Self::spawn_corn_asset).into_configs())
    }
}

pub fn pass_entity(entity: Entity) -> impl FnMut()->Entity {
    Box::new(move || {entity})
}
/// Run condition which tests if a LoaderEntity is currently not loading by checking for a IsLoading Tag component
pub fn loader_is_not_loading(entity: In<Entity>, query: Query<(), Without<IsLoading>>) -> bool{
    query.contains(entity.0)
}


/// Add functionality for adding Loaders to Apps.
pub trait LoadingSystem{
    /// Inserts a Loading Dependency into the app
    fn insert_loader<D: LoaderEntity>(&mut self, loader: D);
    /// Adds a dependent state to the specified loader
    fn register_state_req<S: States+Clone>(&mut self, name: impl Into<String>, state: S);
}
impl LoadingSystem for App{
    fn insert_loader<D: LoaderEntity>(&mut self, loader: D) {
        self.world.spawn((LoaderStatus{name: loader.name(), loaded: false}, loader));
    }
    fn register_state_req<S: States+Clone>(&mut self, name: impl Into<String>, state: S) {
        // Make sure LoadingState exists for S
        self.add_plugins(StateLoadReqPlugin::<S>(PhantomData::<S>::default()));
        // Make sure OnEnter is used to check dependencies for state
        self.world.resource_mut::<StateReqs<S>>().0.insert(state.clone());
        // Get the loader entity, and add the state dependency to the corresponding component
        add_state_req(&mut self.world, name.into(), state);
        // Schedule systems

    }
}
/// Adds a state req to the loader entity specified by name
fn add_state_req<S: States>(world: &mut World, name: String, state: S){
    let mut query = world.query::<(Entity, &LoaderStatus)>();
    let entity = query.iter(world).filter_map(|(entity, status)| {
        if status.name == name {Some(entity)} else {None}
    }).next().expect(format!("Loader Name {} did not correspond to a LoaderStatus Entity. Did you remember to call insert_loader?", name).as_str());
    let mut entity = world.entity_mut(entity);
    if let Some(mut reqs) = entity.get_mut::<StateLoadReq<S>>() {
        reqs.0.insert(state.clone());
    } else {
        entity.insert(StateLoadReq(HashSet::from([state.clone()])));
    }
}

/// System set which runs only when some loading is occuring
#[derive(Debug, Clone, Reflect, PartialEq, Eq, Hash, SystemSet)]
pub struct GlobalLoadingSet;

/// Resource which specifies for a State type, a list of states that have loading dependencies. Used during app creation to add OnEnter Schedules
#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct StateReqs<S: States>(HashSet<S>);
impl<S: States> Default for StateReqs<S> {fn default() -> Self {Self(HashSet::default())}}

#[derive(Debug, Clone)]
pub struct StateLoadReqPlugin<S: States>(PhantomData<S>);
impl<S: States> StateLoadReqPlugin<S>{
    // looks to see if there are any loader entities that need to be loaded for this state
    fn check_loaders(
        loaders: Query<(&LoaderStatus, &StateLoadReq<S>, Option<&IsLoading>)>,
        state: Res<State<S>>,
        mut next_state: ResMut<NextState<LoadingState<S>>>
    ){
        if loaders.into_iter()
            .filter(|(_, reqs, _)| reqs.0.contains(state.get()))
            .any(|(status, _, tag)| tag.is_some() || !status.loaded) 
        {
            next_state.0 = Some(LoadingState::Loading);
        }
    }
    // system which adds IsLoading tag component to loader entities that are required by the current state.
    fn add_tags(
        mut commands: Commands,
        state: Res<State<S>>,
        query: Query<(Entity, &LoaderStatus, &StateLoadReq<S>), Without<IsLoading>>
    ){
        let tags: Vec<(Entity, IsLoading)> = query.into_iter().filter_map(|(entity, status, reqs)| {
            if reqs.0.contains(state.get()) && !status.loaded {Some((entity, IsLoading))} else {None}
        }).collect();
        commands.insert_or_spawn_batch(tags);
    }
    fn finish_loading(
        
        mut next_state: ResMut<NextState<LoadingState<S>>>
    ){

    }
}
impl<S: States> Plugin for StateLoadReqPlugin<S>{
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingState<S>>();
        app.init_resource::<StateReqs<S>>();
        app.register_state_set(StateSystemSet(LoadingState::<S>::Loading));
        app.add_systems(Update, Self::finish_loading.in_set(GlobalLoadingSet).in_set(StateSystemSet(LoadingState::<S>::Loading)));
        app.add_systems(OnEnter(LoadingState::<S>::Loading), (Self::add_tags, LoadingTargetCount::increment));
        app.add_systems(OnExit(LoadingState::<S>::Loading), LoadingTargetCount::decrement);
        app.add_systems(StateTransition, apply_state_transition::<LoadingState<S>>.after(apply_state_transition::<S>));
    }
    fn finish(&self, app: &mut App) {
        let reqs = app.world.remove_resource::<StateReqs<S>>().unwrap();
        for req in reqs.0.into_iter(){
            app.add_systems(OnEnter(req.clone()), Self::check_loaders);
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct LoadingPlugin;
impl Plugin for LoadingPlugin{
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadingTargetCount>();
        app.configure_sets(Update, GlobalLoadingSet.run_if(LoadingTargetCount::is_loading));
    }
}

/// Checks to see if there are any still loading tasks for the current state. when there arent it moves back to loaded state. Also removes IsLoading from completed loaders.
fn finish_loading<S: States>(
    state: Res<State<S>>,
    mut next_state: ResMut<NextState<LoadingState<S>>>,
    query: Query<(Entity, &LoaderStatus, Option<&StateLoadReq<S>>), With<IsLoading>>,
    mut commands: Commands
){
    let state = state.get();
    let mut fully_loaded = true;
    for(entity, status, reqs) in query.into_iter(){
        if status.loaded {commands.entity(entity).remove::<IsLoading>(); continue;}
        let Some(reqs) = reqs else {continue;};
        if reqs.0.contains(state) {fully_loaded = false;}
    }
    if fully_loaded {next_state.0 = Some(LoadingState::Loaded);}
}

/*
    [x] OnEnter(State): We check all load entities to see if any need to be loaded. If so we switch loadingstate->loaded.
    [x] run apply_state_transition afterwards for LoadingState
    [-] OnEnter(LoadingState::Loading): Load entities that are not currently loading have their init functions run, and Loading tags added to them
    [ ] - use a run_if function to check the specific entity for a LoadingTag
    [x] - Use a single function which adds all loading tags.
    [ ] Update: update functions are added to the update schedule with run conditions. global loading cond, in_State(loadingstate) cond, personal should_load condition, runs only once per frame using some component.
    [ ] Observers run check load and remove tag components
    [ ] Cleanup(LoadingState::Loading): See if any Loading entities that need to be loaded are still being loaded. If not LoadingState->Loaded.
    OnExit(LoadingState::Loading): Run cleanup functions for any Load entity with a Loading Tag that is loaded, and remove the tag
    - run_if function to run load entity cleanup functions
    - single function which removes loading tags from loaded entities.

*/

/*


Loaders are an instance of a struct, added as entity components
Loaders have unique names
Loaders have a set of functions that run once when the Loader is asked to load
They have a set of functions that run once per frame while loading
They have a set of functions that run once when they have finished loading
They can be unloaded, and have a set of functions that run when they are unloaded.
They have a set of functions that run once per frame while unloading
They have a set of functions that run once after unloading

'State' Moves to a new State
OnEnter('State')
    A system is run to determine if any dependencies of 'State' are unloaded, moving LoadingState<State> to Loading
    init systemsets of Loading Dependencies are run for loaders that are not loading yet, and are needed.

/// Functionality related to stages of loading for different parts of the app.
/// 
pub trait LoadingStage: Send+Sync+'static+SystemSet+Clone{
    /// Specifies when 
    fn system_set_config() -> Option<SystemSetConfigs>;
    fn end_loading() -> Option<SystemConfigs>;

    /// Name of the loading stage
    fn name() -> String {type_name::<Self>().to_string()}
    
    fn init() -> Option<SystemConfigs> {None}
    fn update() -> Option<SystemConfigs> {None}
    fn finish() -> Option<SystemConfigs> {None}
    
    /// Function which runs during PostUpdate, and is responsible for moving to FINISH_STATE when all tasks are finished
    fn finish_loading(tasks: Query<&LoadingTask<Self>>, mut next_state: ResMut<NextState<Self::State>>) {
        let mut finished = true;
        tasks.into_iter().for_each(|task| finished &= task.completed);
        if !finished {return;}
        next_state.0 = Some(Self::FINISH_STATE);
    }
    /// Function which runs during OnEnter(INIT_STATE), and must switch state to UPDATE_STATE
    fn start_loading(mut next_state: ResMut<NextState<Self::State>>) {
        next_state.0 = Some(Self::UPDATE_STATE);
    }
}



/// Component used to time the run length of Loading Stages
#[derive(Debug, Clone, Reflect, Component)]
pub struct LoadingTimer<S: LoadingStage>{
    start: Instant,
    _phantom_data: PhantomData<S>
}
impl<S: LoadingStage> LoadingTimer<S>{
    pub fn spawn_timer(mut commands: Commands){
        commands.spawn(Self{start: Instant::now(), _phantom_data: PhantomData::default()});
    }
    pub fn stop_timer(timer: Query<(Entity, &LoadingTimer<S>)>, mut commands: Commands){
        let now = Instant::now();
        for (entity, timer) in timer.iter(){
            println!("Loading {} finished in: {}", S::name(), (now-timer.start).as_secs_f32());
            commands.entity(entity).despawn();
        }
    }
}

/// A component representing a single Loading Task for a Loading Stage
#[derive(Default, Debug, Clone, Reflect, Component)]
pub struct LoadingTask<S: LoadingStage>{
    completed: bool,
    name: Option<String>,
    weight: Option<f32>,
    progress: Option<f32>,
    _phantom_data: PhantomData<S>
}

/// A plugin used to add all systems necessary for a specific LoadingStage
#[derive(Debug, Clone)]
pub struct LoadingPlugin<S: LoadingStage>{
    pub timer: bool,
    pub stage: S
}
impl<S: LoadingStage> LoadingPlugin<S>{
    pub fn new(timer: bool, stage: S) -> Self{Self{timer, stage}}
}
impl<S: LoadingStage+Default> Default for LoadingPlugin<S>{
    fn default() -> Self {Self{timer: true, stage: S::default()}}
}
impl<S: LoadingStage> Plugin for LoadingPlugin<S>{
    fn build(&self, app: &mut App) {
        let stage = self.stage.clone();
        app.add_systems(PostUpdate, S::finish_loading.in_set(stage.clone()).run_if(in_state(S::UPDATE_STATE)));
        app.add_systems(OnEnter(S::INIT_STATE), S::start_loading.in_set(stage.clone()));
        if self.timer {
            app.add_systems(OnEnter(S::INIT_STATE), LoadingTimer::<S>::spawn_timer.in_set(stage.clone()));
            app.add_systems(OnEnter(S::FINISH_STATE), LoadingTimer::<S>::stop_timer.in_set(stage.clone()));
        }
        if let Some(spawn) = stage.get_spawn_systems() {
            app.add_systems(OnEnter(S::INIT_STATE), spawn.in_set(stage.clone()));
        }
        if let Some(update) = stage.get_update_systems() {
            app.add_systems(Update, update.in_set(stage.clone()).run_if(in_state(S::UPDATE_STATE)));
        }
        if let Some(cleanup) = stage.get_cleanup_systems() {
            app.add_systems(OnEnter(S::FINISH_STATE), cleanup.in_set(stage.clone()));
        }
        if let Some(spawn) = stage.get_spawn_systems() {
            app.add_systems(OnEnter(S::INIT_STATE), spawn.in_set(stage.clone()));
        }
        if let Some(set_config) = stage.get_set_config() {
            app.configure_sets(Main, set_config);
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct MainMenuLoadingStage;
impl MainMenuLoadingStage{
    pub fn get_plugin(self, timer: bool) -> impl Plugin{LoadingPlugin::new(timer, self)}
}
impl LoadingStage for MainMenuLoadingStage{
    type State = LoadingState;
    const FINISH_STATE: Self::State = LoadingState::Loaded;
    const INIT_STATE: Self::State = LoadingState::Loading;
    const UPDATE_STATE: Self::State = LoadingState::Loading;

    fn name() -> String {"Main Menu".to_string()}
    fn get_set_config() -> Option<SystemSetConfigs> {Some(Self.run_if(in_state(AppStage::MainMenu)))}
}*/
