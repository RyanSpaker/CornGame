#![allow(dead_code)]

use std::marker::PhantomData;
use bevy::{
    utils::hashbrown::{HashMap, HashSet}, 
    ecs::{
        system::{Query, ResMut, Resource}, 
        schedule::{SystemSet, IntoSystemConfigs, IntoSystemSetConfig}, event::{EventWriter, Event}}, 
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
    pub fn ready_for_init(&self) -> bool {
        match self.asset_loading_state {
            AssetState::LOADED => true,
            AssetState::PREPARING => false
        }
    }
}
/*
    Resource
*/

/// System set containing all [State Update Functions](update_corn_field_state) 
/// making sure they run after the [CFSM](CornFieldStateManager) sets up the stale list, 
/// and before it uses that stale list to find stale hashes
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct UpdateCornStateSet;

/// This event means that the specified field is now stale data
#[derive(Event)]
pub struct StaleFieldEvent{
    pub field: RenderableCornFieldID
}


/// ### Corn Field State Manager (CFSM)
/// This resource takes the incoming ```RenderableCornField```s (RCFs) from the main world, and connects the inter-frame state information to them
/// - Holds a list of Hash/ID (u64) -> ```CornFieldState``` pairs.
/// - Updates the state information of each corn field at the beginning of every frame
/// - Manages loading state and visibility state
/// - Loading state is for whether or not the RCF has all of the necessarry assets loaded in order to initialize
/// - Visibility state is for wether or not the RCF can be seen, currently always true, but will be handled by a seperate system later
/// - Also marks old data as stale.
/// - This resource is responsible for loading and stale state transitions.
/// - For now it will also manage visibility, but eventually that will have it's own specialized system which also includes states for billboarding and shell texturing
#[derive(Default, Resource)]
pub struct CornFieldStateManager{
    /// Hashmap of corn field hash id to corn field state.
    /// Contains both asset loading and visibility states
    pub states: HashMap<RenderableCornFieldID, CornFieldState>,
    /// List of hashes that correspond to stale data, 
    pub stale: Vec<(RenderableCornFieldID, CornFieldState)>,
    /// Variable to hold a list of potentially stale hashes
    stale_hashes: HashSet<RenderableCornFieldID>
}
impl CornFieldStateManager{
    /// This is a system that runs before all update_corn_field_state systems, and initally marks all hashes as stale
    /// The update functions then mark all hashes present as not stale
    pub fn setup_stale_hashes(
        mut manager: ResMut<CornFieldStateManager>
    ){
        manager.stale_hashes = HashSet::from_iter(manager.states.clone().into_keys());
    }
    /// This system runs after all update_corn_field_state systems, and marks all stale data as stale, removing them from the states map
    pub fn mark_stale_states(
        mut manager: ResMut<CornFieldStateManager>,
        mut stale_field_event_writer: EventWriter<StaleFieldEvent>
    ){
        manager.stale_hashes.clone().into_iter().for_each(|hash| {
            if let Some(state) = manager.states.remove(&hash){
                manager.stale.push((hash.to_owned(), state));
            }
            stale_field_event_writer.send(StaleFieldEvent { field: hash });
        });
    }
    /// This functions runs during cleanup, and clears the list of stale hashes from the resource
    /// Stale hashes are kept for only the frame when they become stale.
    pub fn cleanup(mut manager: ResMut<CornFieldStateManager>){
        manager.stale.clear();
    }
    /// A function added once for each type of renderable corn field.
    /// This function reads in all corn fields, and makes sure they are in the corn field state manager
    /// also updates any asset_state that isnt loaded yet
    /// Makes sure all corn fields with a hash are not marked as stale
    pub fn update_corn_field_state<T: RenderableCornField>(
        query: Query<(&T, &RenderableCornFieldID)>,
        mut manager: ResMut<Self>
    ){
        // Go through our current hashes, updating the asset state and adding new hashes to the map
        for (field, id) in query.iter(){
            manager.mark_not_stale(id);
            if let Some(state) = manager.states.get_mut(id){
                match state.asset_loading_state{
                    AssetState::PREPARING => {
                        if field.assets_ready(){
                            state.asset_loading_state = AssetState::LOADED;
                        }
                    },
                    _ => {}
                }
            } else {
                manager.states.insert(
                    id.to_owned(), 
                    CornFieldState { 
                        visibility: Visibility::VISIBLE, 
                        asset_loading_state: if field.assets_ready() {AssetState::LOADED} else {AssetState::PREPARING}
                    }
                );
            }
        }
    }
    /// Used by the update systems to mark hashes as not stale
    pub fn mark_not_stale(&mut self, hash: &RenderableCornFieldID){
        self.stale_hashes.remove(hash);
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
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app.add_systems(Render, CornFieldStateManager::update_corn_field_state::<T>.in_set(UpdateCornStateSet));
        }
    }
}

pub struct MasterCornFieldStatePlugin;
impl Plugin for MasterCornFieldStatePlugin{
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app
                .init_resource::<CornFieldStateManager>()
                .add_event::<StaleFieldEvent>()
                .add_systems( Render, (
                    CornFieldStateManager::setup_stale_hashes, CornFieldStateManager::mark_stale_states.after(UpdateCornStateSet)
                ).chain().in_set(RenderSet::Prepare));
            if let Some(schedule) = render_app.get_schedule_mut(Render){
                schedule.configure_set(UpdateCornStateSet.after(CornFieldStateManager::setup_stale_hashes).in_set(RenderSet::Prepare));
            }
        }
    }
}
