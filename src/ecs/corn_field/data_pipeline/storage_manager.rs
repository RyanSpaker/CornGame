#![allow(dead_code)]
use bevy::{
    utils::hashbrown::HashMap, 
    ecs::{system::{Resource, ResMut}, event::{Event, EventReader}, schedule::IntoSystemConfigs}, 
    app::Plugin, 
    render::{RenderApp, RenderSet, Render}
};
use crate::{util::integer_set::{IntegerSet, SubOne}, ecs::corn_field::RenderableCornFieldID};
use super::state_manager::StaleFieldEvent;

/// This type represents a range of positions on the instance buffer. uses a set of integers to do so
pub type BufferRange = IntegerSet<u64>;
impl BufferRange{
    pub fn calculate_continuos_expansion_requirment(&self, domain_end: u64, length: u64)->u64{
        if self.get_continuos(length.to_owned()).is_some() {return 0;}
        if self.end().unwrap_or(0) == domain_end{
            return self.get_endpoint(self.endpoint_count()-2) + length - domain_end;
        }
        return length;
    }
}
impl SubOne for u64{
    fn sub_one(&self) -> Self {
        return self-1;
    }
}

/// ### Corn Buffer Storage Manager (CBSM)
/// This resource holds the storage information of corn fields, the ranges they occupy in the instance buffer. This represents the state of the Instance buffer.
/// Together with the State Manager, we will be able to determine the discrepency, and then the actions necessary to fix it.
/// 
/// This System only needs to react to events sent by other managers, doing none of its own updates
/// - This system holds a list of hashes that represent RCF's currently on the buffer mapped to the ranges they occupy.
/// - If a hash is present in the map, It is loaded on the buffer, and is being rendered (ish)
/// - This means that any initialization code will need to be done in a single frame on the GPU, since we cant have half loaded fields
/// - Also holds a list of available ranges that have no data, and the total size of the instance buffer
#[derive(Default, Resource)]
pub struct CornBufferStorageManager{
    /// Maps Corn Field id to buffer range
    pub ranges: HashMap<RenderableCornFieldID, BufferRange>,
    /// Total set of stale data on the buffer, always 0 since we cleanup stale data during the frame that it is detected
    pub stale_space: BufferRange,
    /// Total free space on the buffer
    pub free_space: BufferRange,
    /// Total length of the buffer in instances
    pub total_space: u64
}
impl CornBufferStorageManager{
    /// Returns whether the specified id is loaded onto the instance buffer
    pub fn contains(&self, id: &RenderableCornFieldID) -> bool{
        self.ranges.contains_key(id)
    }
    /// Reads in stale field events from the State Manager, moving all stale ranges to the stale space value
    pub fn handle_stale_events(
        mut manager: ResMut<CornBufferStorageManager>,
        mut events: EventReader<StaleFieldEvent>
    ){
        let new_stale = BufferRange::union_all(&events.read().filter_map(|ev| manager.ranges.remove(&ev.field)).collect());
        manager.stale_space.union_with(&new_stale);
    }
    /// Moves stale space to free space as specified by the events
    pub fn handle_delete_stale_events(
        mut manager: ResMut<CornBufferStorageManager>,
        mut events: EventReader<DeleteStaleSpaceEvent>
    ){
        let deleted_stale_space = BufferRange::union_all(&events.read().map(|ev| ev.range.to_owned()).collect());
        manager.stale_space.difference_with(&deleted_stale_space);
        manager.free_space.union_with(&deleted_stale_space);
    }
    /// Adds new free space based on the expand space events
    pub fn handle_expand_events(
        mut manager: ResMut<CornBufferStorageManager>,
        mut events: EventReader<ExpandSpaceEvent>
    ){
        let new_space = BufferRange::union_all(&events.read().map(|ev| {
            manager.total_space += ev.length;
            BufferRange::simple(&(manager.total_space - ev.length), &(manager.total_space))
        }).collect());
        manager.free_space.union_with(&new_space);
    }
    /// Removes space from stale and free space, adding it to the ranges hashmap
    pub fn handle_alloc_events(
        mut manager: ResMut<CornBufferStorageManager>,
        mut events: EventReader<AllocSpaceEvent>
    ){
        let taken_space = BufferRange::union_all(&events.read().map(|ev| {
            if let Some(range) = manager.ranges.get_mut(&ev.field){
                range.union_with(&ev.range);
            }else{
                manager.ranges.insert(ev.field.to_owned(), ev.range.to_owned());
            }
            ev.range.to_owned()
        }).collect());
        manager.free_space.difference_with(&taken_space);
        manager.stale_space.difference_with(&taken_space);
    }
    /// Overwrites all values based on a defrag event
    pub fn handle_defrag_events(
        mut manager: ResMut<CornBufferStorageManager>,
        mut events: EventReader<DefragEvent>
    ){
        for ev in events.read(){
            manager.total_space = ev.get_total();
            manager.free_space = ev.free_space.to_owned();
            manager.ranges = HashMap::from_iter(ev.ranges.to_owned().into_iter());
            manager.stale_space = ev.stale_range.to_owned();
        }
    }
    /// Currently unused, but shrinks the buffer by a specified amount from shrink space events
    pub fn handle_shrink_events(
        mut manager: ResMut<CornBufferStorageManager>,
        mut events: EventReader<ShrinkSpaceEvent>
    ){
        let deleted_space = BufferRange::union_all(&events.read().map(|ev| {
            manager.total_space -= ev.length;
            BufferRange::simple(&manager.total_space, &(manager.total_space+ev.length))
        }).collect());
        manager.free_space.difference_with(&deleted_space);
    }
    /// Erases this structs data
    pub fn erase_buffer(&mut self){
        self.ranges = HashMap::new();
        self.stale_space = BufferRange::default();
        self.free_space = BufferRange::default();
        self.total_space = 0;
    }
}

/// This event means that the range specified was just flagged as not stale anymore by direct flagging. Overwrites are handled by allocSpaceEvents
#[derive(Event)]
pub struct DeleteStaleSpaceEvent{
    pub range: IntegerSet<u64>
}
/// This event means that BufferRange was just added as new free space to the buffer. This should usually only be done with simple BufferRanges (contigous ones)
#[derive(Event)]
pub struct ExpandSpaceEvent{
    pub length: u64
}
/// This event means that BufferRange was just written to and filled with data.
#[derive(Event)]
pub struct AllocSpaceEvent{
    pub field: RenderableCornFieldID, 
    pub range: IntegerSet<u64>
}
/// This event means that the buffer was just defragmented. Inside is a vector of id->corresponding buffer range, used to make a new hashmap,
/// as well as a buffer range representing the new free space. This event will override any work done with the dealloc, expand, and reserve events.
/// It is expected that the entire set of ranges is provided in the event.
#[derive(Event)]
pub struct DefragEvent{
    pub ranges: Vec<(RenderableCornFieldID, IntegerSet<u64>)>, 
    pub stale_range: IntegerSet<u64>,
    pub free_space: IntegerSet<u64>
}
impl DefragEvent{
    pub fn get_total(&self) -> u64{
        let mut total: u64 = self.free_space.len() as u64 + self.stale_range.len() as u64;
        for (_, range) in self.ranges.iter(){
            total += range.len() as u64;
        }
        total
    }
}
/// This event means that the buffer was shrunken, and that the provided bufferrange need to be subtracted from the current free space.
/// It is expected the that buffer range is simple (contigous), and corresponds to the end of the array
/// This event is currently unused, as any time we shrink we are also defragmenting, and defrag events override the length of the buffer as well
#[derive(Event)]
pub struct ShrinkSpaceEvent{
    pub length: u64
}

/// Plugin used to add the CornBufferStorageManager to the game
pub struct MasterCornStorageManagerPlugin;
impl Plugin for MasterCornStorageManagerPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<CornBufferStorageManager>()
            .add_event::<DeleteStaleSpaceEvent>()
            .add_event::<ExpandSpaceEvent>()
            .add_event::<AllocSpaceEvent>()
            .add_event::<DefragEvent>()
            .add_event::<ShrinkSpaceEvent>()
            .add_systems(Render, (
                CornBufferStorageManager::handle_stale_events,
                CornBufferStorageManager::handle_delete_stale_events,
                CornBufferStorageManager::handle_expand_events,
                CornBufferStorageManager::handle_alloc_events,
                CornBufferStorageManager::handle_defrag_events,
                CornBufferStorageManager::handle_shrink_events
            ).chain().in_set(RenderSet::Cleanup));
    }
}
