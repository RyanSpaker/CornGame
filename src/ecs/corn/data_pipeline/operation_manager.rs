use std::marker::PhantomData;
use bevy::{
    prelude::*,
    ecs::schedule::SystemSet, 
    render::{RenderSet, RenderApp, Render}
};
use crate::ecs::corn::{buffer::{BufferRange, CornInstanceBuffer}, field::{state::CornFieldState, RenderableCornFieldID}};

pub const READBACK_ENABLED: bool = false;

/// Functionality required by teh CornBufferOperations resource to calculate corn buffer operations. A subset of RenderableCornField
pub trait IntoBufferOperation: Component{
    /// This function returns whether or not the corn field needs its elements to be continously positioned in the buffer
    fn needs_continuos_buffer_space() -> bool {false}
    /// Returns the total number of pieces of corn this field will create
    fn get_instance_count(&self) -> u64;
}


/// Component attached to corn fields representing that the corn field needs to be initialized to a specific range on an instance buffer.
#[derive(Default, Debug, Clone, Component)]
/// We use sparse set because it is more efficient for adding and removing from entities.
#[component(storage = "SparseSet")]
pub struct CornInitOp{
    pub range: BufferRange
}


/// Resource which tracks and contains the operations needed for this frame to correct the instance buffer.
#[derive(Default, Debug, Clone, Resource)]
pub struct CornBufferOperations{
    pub pre_init_state: BufferState,
    pub post_init_state: BufferState,
    pub expansion: u64,
    pub shrink: u64,
    pub defrag: bool,
    pub readback: bool,
    pub init_count: usize,
    avg_range_count: f32
}
#[derive(Default, Debug, Clone)]
pub struct BufferState{
    pub length: u64,
    pub free_space: BufferRange,
    pub stale_space: BufferRange
}
impl CornBufferOperations{
    /// Expands the current post_init_state of the buffer by length, updating the expansion variable as well.
    pub fn expand(&mut self, length: u64){
        self.expansion += length;
        self.post_init_state.free_space.union_with(&BufferRange::simple(&self.post_init_state.length, &(&self.post_init_state.length + length)));
        self.post_init_state.length += length;
    }
    /// Returns whether or not the buffer should shrink
    pub fn should_shrink(&self) -> bool{
        let instance_count = BufferRange::simple(&0, &self.post_init_state.length).difference_with(
            &BufferRange::union(&self.post_init_state.free_space, &self.post_init_state.stale_space)
        ).len() as f32;
        self.post_init_state.length as f32 / instance_count >= 1.75
    }
    /// Returns whether or not the buffer should be defragmented
    pub fn should_defrag(&self) -> bool{
        self.avg_range_count > 2.0
    }
    /// Returns the size of the buffer after the operations are executed
    pub fn get_new_buffer_count(&self) -> u64{
        return self.post_init_state.length;
    }
    /// Returns the total amount of actual instances on the buffer, after the operations have been applied
    pub fn get_taken_space(&self) -> u64{
        BufferRange::simple(&0, &self.post_init_state.length).difference_with(&BufferRange::union(&self.post_init_state.free_space, &self.post_init_state.stale_space)).len()
    }
    /*
        Systems:
    */
    /// Should run during PrepareAssets, after the instance buffer's handle_stale_events function
    /// Sets up the resource to calculate the necessary actions
    pub fn setup(
        mut operations: ResMut<Self>,
        buffer: Res<CornInstanceBuffer>
    ){
        operations.pre_init_state = BufferState{length: buffer.get_instance_count(), free_space: buffer.get_free_space(), stale_space: buffer.get_stale_space()};
        operations.post_init_state = operations.pre_init_state.clone();
        operations.expansion = 0;
        operations.shrink = 0;
        operations.defrag = false;
        operations.readback = false;
        operations.init_count = 0;
        operations.avg_range_count = buffer.ranges.values().map(|r| r.range_count()).sum::<usize>() as f32 / buffer.ranges.len() as f32;
        if operations.avg_range_count.is_nan() {operations.avg_range_count = 0.0;}
    }
    /// Runs for each type of renderable corn field, creates init operations for any field that needs continuos space on the buffer
    pub fn create_continuos_init_operations<T: IntoBufferOperation>(
        mut operations: ResMut<Self>,
        mut commands: Commands,
        buffer: Res<CornInstanceBuffer>,
        query: Query<(Entity, &T, &RenderableCornFieldID, &CornFieldState)>
    ){
        let values: Vec<(Entity, CornInitOp)> = query.iter().filter(|(_, _, id, state)| state.assets_ready() && !buffer.ranges.contains_key(*id)).map(
        |(entity, field, _, _)| 
        {
            // Given all space available for new data, calculate how much the buffer would need to expand to fit our field, could easily be 0 if there is already room.
            let expansion: u64 = BufferRange::union(&operations.post_init_state.free_space, &operations.post_init_state.stale_space)
                .calculate_continuos_expansion_requirment(operations.post_init_state.length, field.get_instance_count());
            //expand by necessary amount
            operations.expand(expansion);
            //reserve space for our data
            let range = BufferRange::union(&operations.post_init_state.free_space, &operations.post_init_state.stale_space).get_continuos(field.get_instance_count()).unwrap();
            operations.post_init_state.free_space.difference_with(&range);
            operations.post_init_state.stale_space.difference_with(&range);
            operations.init_count += 1;
            (entity, CornInitOp{range})
        }).collect();
        commands.insert_or_spawn_batch(values.into_iter());
    }
    /// Runs for each type of renderable corn field, creates init operations for each field that doesnt need continous space on the buffer
    pub fn create_init_operations<T: IntoBufferOperation>(
        mut operations: ResMut<Self>,
        mut commands: Commands,
        buffer: Res<CornInstanceBuffer>,
        query: Query<(Entity, &T, &RenderableCornFieldID, &CornFieldState)>
    ){
        let values: Vec<(Entity, CornInitOp)> = query.iter().filter(|(_, _, id, state)| state.assets_ready() && !buffer.ranges.contains_key(*id)).map(
        |(entity, field, _, _)| 
        {
            let mut count = field.get_instance_count();
            let mut range = BufferRange::default();
            if !operations.post_init_state.stale_space.is_empty(){
                let (stale_range, excess) = operations.post_init_state.stale_space.take(count);
                count = excess;
                range.union_with(&stale_range);
            }
            if !operations.post_init_state.free_space.is_empty(){
                let (free_range, excess) = operations.post_init_state.free_space.take(count);
                count = excess;
                range.union_with(&free_range);
            }
            if count > 0{
                operations.expand(count);
                range.union_with(&operations.post_init_state.free_space.take(count).0);
            }
            operations.init_count += 1;
            (entity, CornInitOp{range})
        }).collect();
        commands.insert_or_spawn_batch(values.into_iter());
    }
    /// Finalizes the operations, figuring out exactly what needs to happen this frame to the buffer
    pub fn finalize_operations(
        mut operations: ResMut<Self>
    ){
        let instance_count = BufferRange::simple(&0, &operations.post_init_state.length).difference_with(
            &BufferRange::union(&operations.post_init_state.free_space, &operations.post_init_state.stale_space)
        ).len();
        if operations.expansion == 0 && operations.init_count == 0 && operations.post_init_state.stale_space.is_empty() && operations.post_init_state.length > 0 && instance_count > 0{
            //Defrag or Shrink operations here
            if operations.should_shrink(){
                let new_buffer_size = instance_count + instance_count/4;
                operations.shrink = (operations.post_init_state.length as i64 - new_buffer_size as i64).max(0) as u64;
                operations.post_init_state.length -= operations.shrink;
                operations.defrag = true;
                operations.readback = READBACK_ENABLED;
            }else if operations.should_defrag(){
                operations.defrag = true;
                operations.readback = READBACK_ENABLED;
            }
        }else if operations.expansion > 0{
            //Expand to 1.25X more than the total instance count
            let new_length = operations.post_init_state.length;
            // if we expand, free space and stale space are 0, meaning t +t/4 - b will always be positive since t == b
            operations.expand((instance_count + instance_count / 4) - new_length);
            operations.readback = READBACK_ENABLED;
        }else if operations.init_count > 0 || !operations.post_init_state.stale_space.is_empty(){
            operations.readback = READBACK_ENABLED;
        }
    }
    
}

/// A System Set for create_continous_init_operations<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct CreateContOperationSet;
/// A System set for create_init_operations<> functions
#[derive(Hash, Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct CreateOperationSet;

/// Adds the Operation Manager functionality to the game
pub struct CornOperationPlugin;
impl Plugin for CornOperationPlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<CornBufferOperations>()
            .add_systems(Render, CornBufferOperations::setup.in_set(RenderSet::PrepareAssets).after(CornInstanceBuffer::handle_stale_events))
            .configure_sets(Render, CreateContOperationSet.after(CornBufferOperations::setup).in_set(RenderSet::PrepareAssets))
            .configure_sets(Render, CreateOperationSet.after(CreateContOperationSet).in_set(RenderSet::PrepareAssets))
            .add_systems(Render, CornBufferOperations::finalize_operations.in_set(RenderSet::PrepareAssets).after(CreateOperationSet));
    }
}

/// Adds operation manager functionality to the game for each type of renderable corn field
pub struct CornFieldOperationPlugin<T: IntoBufferOperation>{
    _marker: PhantomData<T>
}
impl<T: IntoBufferOperation> CornFieldOperationPlugin<T>{
    pub fn new() -> Self {
        CornFieldOperationPlugin { _marker: PhantomData::<T> }
    }
}
impl<T: IntoBufferOperation> Plugin for CornFieldOperationPlugin<T>{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).add_systems(Render, 
        if T::needs_continuos_buffer_space() {
            CornBufferOperations::create_continuos_init_operations::<T>.in_set(CreateContOperationSet)
        } else {
            CornBufferOperations::create_init_operations::<T>.in_set(CreateOperationSet)
        });
    }
}
