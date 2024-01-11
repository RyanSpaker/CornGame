#![allow(dead_code)]

use std::marker::PhantomData;
use bevy::{
    utils::hashbrown::{HashMap, HashSet}, 
    ecs::{
        system::{Query, ResMut, Resource}, 
        schedule::{SystemSet, IntoSystemConfigs, IntoSystemSetConfigs}, event::{EventWriter, Event}}, 
    render::{RenderSet, RenderApp, Render}, 
    app::{Plugin, App}
};
use crate::ecs::corn_field::{RenderableCornField, RenderableCornFieldID};

/*
    State Structs:
*/
/// Represents the visibilit state of the renderable corn fields, either hidden or visible
#[derive(Copy, Clone)]
pub enum Visibility{
    HIDDEN,
    VISIBLE
}

/// Represents the asset state of the renderable corn fields, either preparing, or loaded
#[derive(Copy, Clone)]
pub enum AssetState{
    PREPARING,
    LOADED
}

/// Represents the visibility and asset state of all the renderable corn fields
#[derive(Copy, Clone)]
pub struct CornFieldState{
    visibility: Visibility, 
    asset_loading_state: AssetState
}
impl CornFieldState{
    /// Returns whether the asset state is in the loaded state
    pub fn assets_loaded(&self) -> bool {
        match self.asset_loading_state {
            AssetState::LOADED => true,
            AssetState::PREPARING => false
        }
    }
    /// Returns a state enum with visibility set to true and asset state set with the bool parameter
    pub fn visible_from_asset_state(asset_state: bool) -> Self{
        Self { visibility: Visibility::VISIBLE, asset_loading_state: if asset_state {AssetState::LOADED} else {AssetState::PREPARING} }
    }
}

/*
    Resource
*/

/// System set containing all [State Update Functions](update_corn_field_state) for each type of corn field
/// making sure they run after the [CFSM](CornFieldStateManager) sets up the stale list, 
/// and before it uses that stale list to find stale hashes
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct UpdateCornStateSet;

/// This event means that the specified field is now stale data
#[derive(Debug, Clone, Event)]
pub struct StaleFieldEvent{
    pub field: RenderableCornFieldID
}
impl From<RenderableCornFieldID> for StaleFieldEvent{
    fn from(value: RenderableCornFieldID) -> Self {
        Self{field: value}
    }
}

/// This event means that the specified field is new, and wasn't seen last frame
#[derive(Debug, Clone, Event)]
pub struct NewFieldEvent{
    pub field: RenderableCornFieldID
}
impl From<RenderableCornFieldID> for NewFieldEvent{
    fn from(value: RenderableCornFieldID) -> Self {
        Self{field: value}
    }
}


/// ### Corn Field State Manager (CFSM)
/// This resource takes the incoming ```RenderableCornField```s (RCFs) from the main world, and connects the inter-frame state information to them
/// - Holds a list of Hash/ID (u64) -> ```CornFieldState``` pairs.
/// - Updates the state information of each corn field at the beginning of every frame
/// - Manages loading state and visibility state
/// - Loading state is for whether or not the RCF has all of the necessarry assets loaded in order to initialize
/// - Visibility state is for whether or not the RCF can be seen, currently always true, but will be handled by a seperate system later
/// - Also determines stale data, and sends events any time data is determined as stale
/// - This resource is responsible for loading and stale state transitions.
/// - For now it will also manage visibility, but eventually that will have it's own specialized system which also includes states for billboarding and shell texturing
#[derive(Default, Resource)]
pub struct CornFieldStateManager{
    /// Hashmap of corn field hash id to corn field state.
    /// Contains both asset loading and visibility states
    pub states: HashMap<RenderableCornFieldID, CornFieldState>,
    /// Temporary storage for stale corn field id's
    /// Initially holds all id's, but each update state function removes real corn field id's from the list
    /// At the end of the update cycle, only stale hash id's remain
    stale_hashes: HashSet<RenderableCornFieldID>,
    /// Temporary Storage for new corn field id's
    /// Starts out empty, any corn field not in the stale hashes is new, since it starts out with every known id
    /// new corn field events are sent using this info at the end of the update cycle
    new_hashes: HashSet<RenderableCornFieldID>
}
impl CornFieldStateManager{
    /// Returns whether or not a specific field id is ready to render
    pub fn is_ready(&self, id: &RenderableCornFieldID) -> bool{
        self.states.get(id).is_some_and(|state| state.assets_loaded())
    }
    /// Runs once per corn field type, updating state information for each corn field
    pub fn update_state<T: RenderableCornField>(
        query: Query<(&T, &RenderableCornFieldID)>,
        mut manager: ResMut<Self>
    ){
        // Go through our current hashes, updating the asset state and adding new hashes to the map
        // Remove real hashes from stale hash set, add new hashes to new hash set
        query.iter().for_each(|(field, id)| {
            if !manager.stale_hashes.remove(id){
                manager.new_hashes.insert(id.clone());
            }
            if let Some(state) = manager.states.get_mut(id){
                if !state.assets_loaded() && field.assets_ready(){
                    state.asset_loading_state = AssetState::LOADED;
                }
            }else{
                manager.states.insert(id.clone(), CornFieldState::visible_from_asset_state(field.assets_ready()));
            }
        });
    }
    /// Runs after all update_state functions, finishing state info, sending out new ans stale field events, and reseting stale and new hash sets for next frame
    pub fn finish_update_cycle(
        mut manager: ResMut<Self>,
        mut stale_events: EventWriter<StaleFieldEvent>,
        mut new_events: EventWriter<NewFieldEvent>
    ){
        stale_events.send_batch(manager.stale_hashes.drain().map(|id| id.into()));
        new_events.send_batch(manager.new_hashes.drain().map(|id| id.into()));
        manager.stale_hashes = HashSet::from_iter(manager.states.keys().cloned());
    }
    /// Adds the systems this resource needs into the app
    pub fn add_systems(app: &mut App){
        app
            .configure_sets(Render, UpdateCornStateSet.in_set(RenderSet::PrepareAssets))
            .add_systems(Render, Self::finish_update_cycle.after(UpdateCornStateSet).in_set(RenderSet::PrepareAssets));
    }
}

/*
    Plugins:
*/

/// A plugin to add the update_corn_field_state function for each renderable corn field type
pub struct CornFieldStatePlugin<T: RenderableCornField>{
    _marker: PhantomData<T>
}
impl<T: RenderableCornField> CornFieldStatePlugin<T>{
    pub fn new() -> Self {
        CornFieldStatePlugin { _marker: PhantomData::<T> }
    }
}
impl<T: RenderableCornField> Plugin for CornFieldStatePlugin<T>{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).add_systems(Render, 
            CornFieldStateManager::update_state::<T>.in_set(UpdateCornStateSet));
    }
}

pub struct MasterCornFieldStatePlugin;
impl Plugin for MasterCornFieldStatePlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_event::<StaleFieldEvent>()
            .add_event::<NewFieldEvent>()
            .init_resource::<CornFieldStateManager>();
        CornFieldStateManager::add_systems(app.sub_app_mut(RenderApp));
    }
}
