use std::{marker::PhantomData, any::type_name};
use bevy::{
    prelude::*, 
    utils::hashbrown::HashMap, 
    ecs::schedule::SystemSet, 
    render::{RenderSet, RenderApp, Render}
};
use crate::ecs::corn_field::RenderableCornFieldID;
use super::{
    state_manager::{CornFieldStateManager, StaleFieldEvent}, 
    storage_manager::{CornBufferStorageManager, BufferRange, AsBufferRange},
    RenderableCornField
};

/*
    Corn Buffer Operation Calculator
*/

/// A System Set for create_continous_init_operations<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct CreateContOperationSet;
/// A System set for create_init_operations<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct CreateOperationSet;
/// A resource which calculates and stores the actions necessary to keep the corn buffer aligned to the game state
#[derive(Default, Resource, Debug)]
pub struct CornBufferOperationCalculator{
    /// The true length of the buffer this frame
    pub current_buffer_length: u64,
    /// The length of the buffer after any expand or shrink operations are completed
    pub new_buffer_length: u64,
    /// The total number of instances the buffer should be expanded by
    pub expansion: u64,
    /// The total number of instances the buffer should be shrunken by
    pub shrink: u64,
    /// A hashmap of cornfield id to BufferRange where it should be intialized
    pub init_ops: HashMap<RenderableCornFieldID, (BufferRange, String)>,
    /// Whether or not the buffer should be defragged. If there is any shrinking, this will be true.
    pub defrag: bool,
    /// Whether or not the buffer should be read back to the cpu after the operations, true during development and only if there are operations to be done
    pub readback: bool,
    /// The range of stale positions in the buffer after the operations have been applied (sans stale flagging, which will reset these positions)
    pub new_stale_space: BufferRange,
    /// The total free space in the buffer after the operations have been applied
    pub new_free_space: BufferRange,
    /// The avg # of continuos ranges per corn field in the buffer, used to calculate if the buffer needs to be defragmented
    pub avg_range_count: f32
}
impl CornBufferOperationCalculator{
    /// Sets up the manager for this frame by resetting the conditions, and preparing to find the necessary actions
    pub fn determine_initial_conditions(
        mut manager: ResMut<CornBufferOperationCalculator>,
        storage: Res<CornBufferStorageManager>,
        mut stale_events: EventReader<StaleFieldEvent>
    ){
        manager.current_buffer_length = storage.total_space;
        manager.new_buffer_length = manager.current_buffer_length;
        manager.expansion = 0;
        manager.shrink = 0;
        manager.init_ops = HashMap::default();
        manager.defrag = false;
        manager.readback = false;
        manager.new_stale_space = storage.stale_space.to_owned();
        manager.new_free_space = storage.free_space.to_owned();
        // add any newly stale data that is currently loaded to the stale ranges
        manager.new_stale_space.union_with(&BufferRange::union_all(
            &stale_events.iter().filter_map(|ev| {
                storage.ranges.get(&ev.field).and_then(|range| Some(range.to_owned()))
            }).collect()
        ));
        manager.avg_range_count = storage.ranges.values().map(|range| range.range_count()).sum::<usize>() as f32 / storage.ranges.len() as f32;
    }
    /// Runs for each type of renderable corn field, creates init operations for any field that needs continuos space on the buffer
    pub fn create_continuos_init_operations<T: RenderableCornField>(
        mut manager: ResMut<CornBufferOperationCalculator>,
        storage: Res<CornBufferStorageManager>,
        states: Res<CornFieldStateManager>,
        query: Query<(&T, &RenderableCornFieldID)>
    ){
        query.iter()
            .filter(|(_, id)| 
                states.states.get(*id).is_some_and(|state| state.ready_for_init()) && 
                storage.ranges.get(*id).is_none()
            ).for_each(|(field, id)| 
        {
            manager.reserve_continuos(id.to_owned(), field.get_instance_count(), type_name::<T>().to_string());
        });
    }
    /// Runs for each type of renderable corn field, creates init operations for each field that doesnt need continous space on the buffer
    pub fn create_init_operations<T: RenderableCornField>(
        mut manager: ResMut<CornBufferOperationCalculator>,
        storage: Res<CornBufferStorageManager>,
        states: Res<CornFieldStateManager>,
        query: Query<(&T, &RenderableCornFieldID)>
    ){
        query.iter()
            .filter(|(_, id)| 
                states.states.get(*id).is_some_and(|state| state.ready_for_init()) && 
                storage.ranges.get(*id).is_none()
            ).for_each(|(field, id)| 
        {
            if T::needs_continuos_buffer_space(){
                manager.reserve_continuos(id.to_owned(), field.get_instance_count(), type_name::<T>().to_string());
            }else{
                let mut count = field.get_instance_count();
                let mut range = BufferRange::default();
                if !manager.new_stale_space.is_empty(){
                    let (stale_range, excess) = manager.new_stale_space.take(count);
                    count = excess;
                    range.union_with(&stale_range);
                }
                if !manager.new_free_space.is_empty(){
                    let (free_range, excess) = manager.new_free_space.take(count);
                    count = excess;
                    range.union_with(&free_range);
                }
                if count > 0{
                    manager.expand(count);
                    range.union_with(&&manager.new_free_space.take(count).0);
                }
                manager.init_ops.insert(id.to_owned(), (range, type_name::<T>().to_string()));
            }
        });
    }
    /// Finalizes the operations, figuring out exactly what needs to happen this frame to the buffer
    pub fn finalize_operations(
        mut manager: ResMut<CornBufferOperationCalculator>
    ){
        if manager.expansion == 0 && manager.init_ops.is_empty() && manager.new_stale_space.is_empty(){
            if manager.new_buffer_length != 0 {
                let instance_count = BufferRange::simple(&0, &manager.new_buffer_length).difference_with(
                    &BufferRange::union(&manager.new_free_space, &manager.new_stale_space)
                ).len();
                if instance_count > 0 {
                    //Defrag or Shrink operations here
                    if manager.should_shrink(){
                        manager.defrag = true;
                        let instance_count = BufferRange::simple(&0, &manager.new_buffer_length).difference_with(
                            &BufferRange::union(&manager.new_free_space, &manager.new_stale_space)
                        ).len();
                        let new_buffer_size = instance_count + instance_count/4;
                        manager.shrink = (manager.new_buffer_length as i64 - new_buffer_size as i64).max(0) as u64;
                        manager.new_buffer_length -= manager.shrink;
                        manager.readback = true;
                    }else if manager.should_defrag(){
                        manager.defrag = true;
                        manager.readback = true;
                    }
                }
            }
        }else if manager.expansion > 0{
            //Expand to 1.25X more than the total instance count
            let instance_count = BufferRange::simple(&0, &manager.new_buffer_length).difference_with(
                &BufferRange::union(&manager.new_free_space, &manager.new_stale_space)
            ).len();
            let buffer_length: u64 = manager.new_buffer_length;
            // if we expand, free space and stale space are 0, meaning t +t/4 - b will always be positive since t == b
            manager.expand((instance_count + instance_count / 4) - buffer_length);
            manager.readback = true;
        }else{
            // this else block only gets run when init ops inst empty, or when stale space isnt empty, so readback is needed
            manager.readback = true;
        }
    }
    /// Returns whether or not the buffer should shrink
    pub fn should_shrink(&self) -> bool{
        let instance_count = BufferRange::simple(&0, &self.new_buffer_length).difference_with(
            &BufferRange::union(&self.new_free_space, &self.new_stale_space)
        ).len() as f32;
        self.new_buffer_length as f32 / instance_count >= 1.75
    }
    /// Returns whether or not the buffer should be defragmented
    pub fn should_defrag(&self) -> bool{
        self.avg_range_count > 2.0
    }
    /// Reserves a continous block of space for a corn field
    pub fn reserve_continuos(&mut self, id: RenderableCornFieldID, length: u64, typename: String){
        let expansion: u64 = BufferRange::union(&self.new_free_space, &self.new_stale_space)
            .calculate_continuos_expansion_requirment(self.new_buffer_length, length);
        self.expand(expansion);
        if let Some(range) = BufferRange::union(&self.new_free_space, &self.new_stale_space).get_continuos(length){
            self.new_free_space.difference_with(&range);
            self.new_stale_space.difference_with(&range);
            self.init_ops.insert(id, (range, typename));
        }
    }
    /// Expands the buffer by length
    pub fn expand(&mut self, length: u64){
        self.expansion += length;
        self.new_free_space.union_with(&BufferRange::simple(&self.new_buffer_length, &(&self.new_buffer_length + length)));
        self.new_buffer_length += length;
    }
    /// Returns the size of the buffer after the operations are executed
    pub fn get_new_buffer_count(&self) -> u64{
        return self.new_buffer_length;
    }
    /// Returns the total amount of actual instances on the buffer, after the operations have been applied
    pub fn get_taken_space(&self) -> u64{
        BufferRange::simple(&0, &self.new_buffer_length).difference_with(&BufferRange::union(&self.new_free_space, &self.new_stale_space)).len()
    }
}

/// Adds the Operation Manager functionality to the game
pub struct MasterCornOperationPlugin;
impl Plugin for MasterCornOperationPlugin{
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app.init_resource::<CornBufferOperationCalculator>()
                .add_systems(Render, (
                    CornBufferOperationCalculator::determine_initial_conditions.after(CornFieldStateManager::mark_stale_states),
                    CornBufferOperationCalculator::finalize_operations.after(CreateOperationSet)
                ).in_set(RenderSet::Prepare));
            if let Some(schedule) = render_app.get_schedule_mut(Render){
                schedule.configure_set(CreateContOperationSet.after(CornBufferOperationCalculator::determine_initial_conditions).in_set(RenderSet::Prepare));
                schedule.configure_set(CreateOperationSet.after(CreateContOperationSet).in_set(RenderSet::Prepare));
            }
        }
    }
}

/// Adds operation manager functionality to the game for each type of renderable corn field
pub struct CornOperationPlugin<T: RenderableCornField>{
    _marker: PhantomData<T>
}
impl<T: RenderableCornField> CornOperationPlugin<T>{
    pub fn new() -> Self {
        CornOperationPlugin { _marker: PhantomData::<T> }
    }
}
impl<T: RenderableCornField> Plugin for CornOperationPlugin<T>{
    fn build(&self, app: &mut App) {
        if let Ok(render_app) = app.get_sub_app_mut(RenderApp){
            render_app.add_systems(Render, CornBufferOperationCalculator::create_init_operations::<T>.in_set(CreateOperationSet));
            if T::needs_continuos_buffer_space(){
                render_app.add_systems(Render, CornBufferOperationCalculator::create_continuos_init_operations::<T>.in_set(CreateContOperationSet));
            }
        }
    }
}
