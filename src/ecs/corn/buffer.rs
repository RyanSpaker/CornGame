use bevy::{prelude::*, render::{render_asset::RenderAssets, render_resource::*, renderer::RenderDevice, Render, RenderApp, RenderSet}, utils::hashbrown::HashMap};
use bytemuck::{Pod, Zeroable};
use crate::util::integer_set::{IntegerSet, SubOne};
use super::{asset::{CornAsset, CornModel}, field::{state::{PreviousFrameCornFields, StaleCornFieldEvent}, RenderableCornFieldID}};

/// Struct representing the Per Corn Stalk data on  the GPU
#[derive(Clone, Copy, Pod, Zeroable, Debug, ShaderType)]
#[repr(C)]
pub struct PerCornData{
    /// Offset from the origin for this piece of corn.
    offset: Vec3,
    /// Scale of this corn stalk
    scale: f32,
    /// Rotation of this corn stalk in the form <sin(theta), cos(theta)>
    rotation: Vec2,
    /// an id, not used by most corn fields, but can be used to signify special traits
    uuid: u32,
    /// whether or not the corn piece should be rendered
    enabled: u32
}
/// A value representing the total number of bytes per corn stalk in the gpu buffer. equal to the size of [`PerCornData`]
pub const CORN_DATA_SIZE: u64 = 32;

/// This type represents a range of positions on the instance buffer. uses a [`IntegerSet`] of u64 to do so
pub type BufferRange = IntegerSet<u64>;
impl BufferRange{
    /// This function calculates how much the domain would need to expand rightward in order to have a contiguos range of `length`.
    /// 
    /// This function is used to calculate how much a buffer would need to expand to fit a contigous range of `length`, assuming 
    /// that this Range represents the buffers free space, and domain_end is the length of the buffer.
    /// 
    /// If this range already has a contiguous range of `length`, return 0, as no expansion is necessary.
    /// 
    /// If this range does not have a contigous range of `length`, and doesn't contain domain_end-1 (the last element in the domain). 
    /// The function returns `length` as the domain needs to expand by length, adding the values (domain_end..domain_end + `length`) as the contigus range.
    /// 
    /// Finally, if the range does not have a contigous range of `length`, but does contain a range containing domain_end, then the expansion is domain_end - length_of_range, 
    /// as the expanded space would concatenate to the mentioned range, and thus less expansion is necessary.
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

/// A resource which holds a Corn Instance Buffer
#[derive(Default, Debug, Clone, Resource)]
pub struct CornInstanceBuffer{
// Buffers
    /// Buffer which holds the base data
    data_buffer: Option<Buffer>,
    /// Buffer which holds the corn data after frustum culling and lod sorting
    sorted_buffer: Option<Buffer>,
    /// Buffer which holds the indirect draw command data for our corn mesh
    indirect_buffer: Option<Buffer>,
// Data info
    /// Total number of corn positions on the buffer.
    data_count: u64,
    /// Total number of LOD's recognized by the indirect buffer
    pub lod_count: u64,
    /// Unique id for this buffer which is altered after any buffer changes. 
    /// 
    /// Useful for knowing if the buffer pointers have changed location.
    pub time_id: u64,
// Data Map Info
    /// Maps Corn Field id to buffer range
    pub ranges: HashMap<RenderableCornFieldID, BufferRange>,
    /// Total set of stale data on the buffer
    stale_space: BufferRange,
    /// Total free space on the buffer
    free_space: BufferRange
}
impl CornInstanceBuffer{
    /// Returns whether or not the corn data is ready to begin rendering
    pub fn ready_to_render(&self) -> bool{
        self.data_buffer.is_some() && self.indirect_buffer.is_some() && !self.ranges.is_empty()
    }
    /// Returns the number of instance position in the buffer
    pub fn get_instance_count(&self) -> u64 {return self.data_count;}
    /// Returns the data buffer
    pub fn get_instance_buffer(&self) -> Option<&Buffer> {return self.data_buffer.as_ref()}
    /// Returns the sorted data buffer
    pub fn get_sorted_buffer(&self) -> Option<&Buffer> {return self.sorted_buffer.as_ref()}
    /// Returns the indirect buffer
    pub fn get_indirect_buffer(&self) -> Option<&Buffer> {return self.indirect_buffer.as_ref()}
    /// Returns the set of empty position in the buffer
    pub fn get_free_space(&self) -> BufferRange {return self.free_space.clone();}
    /// Returns the set of stale data positions in the buffer
    pub fn get_stale_space(&self) -> BufferRange {return self.stale_space.clone();}
    /// Returns whether the specified id is loaded onto the instance buffer
    pub fn contains(&self, id: &RenderableCornFieldID) -> bool{
        self.ranges.contains_key(id)
    }
    /// Removes a set of integers from the stale set into the free space set.
    /// Used when a system has cleared stale data off of the buffer
    pub fn delete_stale_range(&mut self, freed_range: BufferRange){
        self.stale_space.difference_with(&freed_range);
        self.free_space.difference_with(&freed_range);
    }
    /// Extends the buffer space by length, adding the new space to the free space set
    pub fn expand_space(&mut self, length: u64){
        self.free_space.union_with(&BufferRange::simple(&self.data_count, &(self.data_count+length)));
    }
    /// Removes a range from free and stale space, reserving it for a specific corn field id
    pub fn alloc_space(&mut self, range: BufferRange, id: RenderableCornFieldID){
        self.free_space.difference_with(&range);
        self.stale_space.difference_with(&range);
        if let Some(cur_range) = self.ranges.get_mut(&id){
            cur_range.union_with(&range);
        }else{
            self.ranges.insert(id, range);
        }
    }
    /// Shrinks the free space by a specific length. Assumes the buffer is empty from end-length -> end
    pub fn shrink_space(&mut self, length: u64){
        self.free_space.difference_with(&BufferRange::simple(&(self.data_count-length), &self.data_count));
    }
    /// Replaces buffer map data with new data, used by the defrag system after a defrag pass.
    pub fn defrag(&mut self, new_ranges: Vec<(RenderableCornFieldID, IntegerSet<u64>)>, new_stale: IntegerSet<u64>, new_free: IntegerSet<u64>){
        self.free_space = new_free;
        self.stale_space = new_stale;
        self.ranges = HashMap::from_iter(new_ranges.into_iter());
    }
    /// Replace the data buffer of this resource with a new one, remaking the sorted buffer as well.
    /// 
    /// new_data: new buffer, new_count: max corn instance count in new buffer
    pub fn swap_instance_buffer(&mut self, new_data: Buffer, new_count: u64, render_device: &RenderDevice) {
        self.data_buffer.replace(new_data).and_then(|old_buffer| Some(old_buffer.destroy()));
        self.sorted_buffer.replace(render_device.create_buffer(&BufferDescriptor{
            label: Some("Corn Instance Index Buffer".into()),
            size: new_count*CORN_DATA_SIZE,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
            mapped_at_creation: false
        })).and_then(|old_buffer| Some(old_buffer.destroy()));
        self.data_count = new_count;
        self.time_id += 1;
    }
    
    /*
        Systems:
    */

    /// Runs during cleanup, making sure the resource has a correct indirect buffer when needed
    pub fn update_indirect_buffer(
        mut instance_buffer: ResMut<Self>,
        render_device: Res<RenderDevice>,
        corn: Res<CornModel>,
        corn_model: Res<RenderAssets<CornAsset>>
    ){
        // Leave early if we have no corn data, the corn mesh isn't loaded yet, or if we already have a working indirect buffer
        if instance_buffer.ranges.is_empty() || !corn.loaded || (instance_buffer.lod_count == corn.lod_count as u64 && instance_buffer.indirect_buffer.is_some()) {return;}
        instance_buffer.lod_count = corn.lod_count as u64;
        instance_buffer.time_id += 1;
        let corn_meshes = corn_model.get(&corn.asset).unwrap();
        let data: Vec<u32> = (0..instance_buffer.lod_count).into_iter()
            .flat_map(|i| [
                corn_meshes.lod_data[i as usize].total_vertices as u32, 
                0, 
                corn_meshes.lod_data[i as usize].start_vertex as u32, 
                0, 
                0
            ].into_iter()).collect();
        // Replace indirect buffer
        instance_buffer.indirect_buffer.replace(render_device.create_buffer_with_data(&BufferInitDescriptor { 
            label: Some("Corn Indirect Buffer".into()), 
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_SRC, 
            contents: bytemuck::cast_slice(&data[..]) 
        })).and_then(|old_buffer| Some(old_buffer.destroy()));
    }
    /// Runs during cleanup, deletes the buffers if no corn is in them
    pub fn cleanup(
        mut instance_buffer: ResMut<Self>
    ){
        if !instance_buffer.ranges.is_empty() {return;}
        instance_buffer.data_buffer.take().and_then(|b| Some(b.destroy()));
        instance_buffer.sorted_buffer.take().and_then(|b| Some(b.destroy()));
        instance_buffer.indirect_buffer.take().and_then(|b| Some(b.destroy()));
        instance_buffer.data_count = 0;
        instance_buffer.lod_count = 0;
        instance_buffer.time_id += 1;
        instance_buffer.stale_space = BufferRange::default();
        instance_buffer.free_space = BufferRange::default();
        instance_buffer.ranges = HashMap::default();
    }
    /// Runs in PrepareAssets after the send field events function
    /// Reads in stale field events from the State Manager, moving all stale ranges to the stale space value
    pub fn handle_stale_events(
        mut manager: ResMut<Self>,
        mut events: EventReader<StaleCornFieldEvent>
    ){
        let new_stale = BufferRange::union_all(&events.read().filter_map(|ev| manager.ranges.remove(&ev.field)).collect());
        manager.stale_space.union_with(&new_stale);
    }
}

/// Adds Functionality relevant to the corn instance buffer to the app
pub struct CornBufferPlugin;
impl Plugin for CornBufferPlugin{
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<CornInstanceBuffer>()
            .add_systems(Render, (
                CornInstanceBuffer::handle_stale_events.in_set(RenderSet::PrepareAssets).after(PreviousFrameCornFields::send_field_events),
                (CornInstanceBuffer::cleanup, CornInstanceBuffer::update_indirect_buffer).in_set(RenderSet::Cleanup)
            ));
    }
}
