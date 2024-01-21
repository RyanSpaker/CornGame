use std::marker::PhantomData;
use bevy::{
    utils::hashbrown::HashSet, 
    ecs::{
        component::Component, entity::Entity, event::{EventWriter, Event}, schedule::IntoSystemConfigs, system::{Commands, Local, Query, ResMut, Resource}}, 
    render::{Extract, ExtractSchedule, Render, RenderApp, RenderSet}, 
    app::{Plugin, App}
};
use super::RenderableCornFieldID;

/// A trait containing all functionality needed for corn field state. Is a subset of RenderableCornField functionality
pub trait CornAssetState: Component{
    /// This function returns true when the corn field is ready for initialization, and all assets needed are loaded.
    /// If a corn field type needs more resources in order to tell if it is ready, it is responsible for adding a tracked variable which is updated in a bevy system every frame.
    fn assets_ready(&self) -> bool {true}
}

/// A component containing state information for a corn field
#[derive(Default, Debug, Clone, Component)]
pub struct CornFieldState{
    /// whether the assets needed by the corn field for initialization and/or rendering have been loaded
    assets_loaded: bool
}
impl CornFieldState{
    /// Returns whether the assets of this corn field entity are loaded
    pub fn assets_ready(&self) -> bool {self.assets_loaded}
    /// Sets the asset loading state
    pub fn set_asset_state(&mut self, ready: bool) {self.assets_loaded = ready;}
    /*
    Systems:
    */
    /// This function is responsible for putting the correct state information in the CornFieldState component
    pub fn update_state<T: CornAssetState>(
        mut fields: Query<(&T, &mut CornFieldState)>
    ){
        fields.iter_mut().for_each(|(field, mut state)| state.set_asset_state(field.assets_ready()));
    }
    /// Function responsible for adding corn field state components to corn field entities in thge extract schedule
    pub fn add_state_component<T: CornAssetState>(
        mut commands: Commands,
        mut previous_len: Local<usize>,
        query: Extract<Query<(Entity, &T)>>
    ){
        let mut values = Vec::with_capacity(*previous_len);
        for (entity, _) in &query {
            values.push((entity, CornFieldState::default()));
        }
        *previous_len = values.len();
        commands.insert_or_spawn_batch(values);
    }
}



/// This event means that the specified field is now stale data
#[derive(Debug, Clone, Event)]
pub struct StaleCornFieldEvent{
    pub field: RenderableCornFieldID
}
impl From<RenderableCornFieldID> for StaleCornFieldEvent{
    fn from(value: RenderableCornFieldID) -> Self {
        Self{field: value}
    }
}
/// This event means that the specified field is new, and wasn't seen last frame
#[derive(Debug, Clone, Event)]
pub struct NewCornFieldEvent{
    pub field: RenderableCornFieldID
}
impl From<RenderableCornFieldID> for NewCornFieldEvent{
    fn from(value: RenderableCornFieldID) -> Self {
        Self{field: value}
    }
}

/// A resource which holds the id's of every corn field present in the game the frame prior. Useful for tracking stale and new corn fields
#[derive(Default, Debug, Clone, Resource)]
pub struct PreviousFrameCornFields{
    /// The set of ids of each corn field present in the prior frame.
    ids: HashSet<RenderableCornFieldID>
}
impl PreviousFrameCornFields{
    /// Bevy System which should run once every frame in the RenderApp during PrepareAssets
    /// This system is responsible for sending out new and stale field events using the previous frames corn fields as a reference
    pub fn send_field_events(
        mut prev_fields: ResMut<PreviousFrameCornFields>,
        fields: Query<&RenderableCornFieldID>,
        mut new_event_writer: EventWriter<NewCornFieldEvent>,
        mut stale_event_writer: EventWriter<StaleCornFieldEvent>
    ){
        let cur_ids: HashSet<RenderableCornFieldID> = HashSet::from_iter(fields.iter().cloned());
        new_event_writer.send_batch(cur_ids.difference(&prev_fields.ids).into_iter().map(|v| NewCornFieldEvent { field: v.clone() }));
        stale_event_writer.send_batch(prev_fields.ids.difference(&cur_ids).into_iter().map(|v| StaleCornFieldEvent { field: v.clone() }));
        prev_fields.ids = cur_ids;
    }
}


/// Plugins to add Corn Field State Functionality to the app for each type of corn field

pub struct CornFieldStatePlugin<T: CornAssetState>{
    _marker: PhantomData<T>
}
impl<T: CornAssetState> CornFieldStatePlugin<T>{
    pub fn new() -> Self {
        CornFieldStatePlugin { _marker: PhantomData::<T> }
    }
}
impl<T: CornAssetState> Plugin for CornFieldStatePlugin<T>{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_systems(Render, CornFieldState::update_state::<T>.in_set(RenderSet::PrepareAssets))
            .add_systems(ExtractSchedule, CornFieldState::add_state_component::<T>);
    }
}

pub struct MasterCornFieldStatePlugin;
impl Plugin for MasterCornFieldStatePlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<PreviousFrameCornFields>()
            .add_event::<NewCornFieldEvent>()
            .add_event::<StaleCornFieldEvent>()
            .add_systems(Render, PreviousFrameCornFields::send_field_events.in_set(RenderSet::PrepareAssets));
    }
}
