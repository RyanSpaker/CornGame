use bevy::{prelude::*, render::{
    render_resource::Buffer, renderer::{RenderDevice, RenderQueue}, 
    sync_world::RenderEntity, Extract, Render, RenderApp, RenderSet
}};
use wgpu::{util::BufferInitDescriptor, BufferUsages};
use crate::util::extract_changed::ExtractChangedComponentPlugin;
use super::{asset::CornLodInfo, CornField};

/// Cutoffs for all corn fields without special settings
#[derive(Debug, Clone, PartialEq, Reflect, Resource)]
#[reflect(Resource)]
pub struct GlobalLodCutoffs(pub Vec<f32>);
impl Default for GlobalLodCutoffs{fn default() -> Self {Self(vec![5.0, 10.0, 20.0, 40.0, 80.0, 500.0])}}
impl GlobalLodCutoffs{
    /// Places LodCutoffs on render corn fields that have no local cutoffs
    fn extract_to_local(
        mut commands: Commands,
        query: Extract<Query<RenderEntity, (With<CornField>, Without<LodCutoffs>)>>,
        render_data: Query<&LodCutoffs>,
        res: Extract<Res<Self>>
    ) {
        let mut values = vec![];
        for entity in &query {
            // Dont push unchanged data
            if render_data.get(entity).is_ok_and(|c| c.0 == res.0) {continue;}
            values.push((entity, LodCutoffs(res.0.clone())));
        }
        commands.try_insert_batch(values);
    }
}

/// Per corn field cutoffs. Filled with Global lod cutoffs when extracting
#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct LodCutoffs(pub Vec<f32>);
impl LodCutoffs{
    /// Calculates some number of cutoffs from a source by shrinking or extending the cutoffs
    fn resize_cutoffs(&self, count: usize) -> Vec<f32> {
        let mut cutoffs = self.0.clone();
        while cutoffs.len() < count {cutoffs.push(cutoffs.last().cloned().unwrap_or(5.0));}
        cutoffs[..count].to_vec()
    }
}

/// Component on render app corn fields which holds the lod cutoff buffer
#[derive(Debug, Clone, Component)]
pub struct LodCutoffBuffer(pub Buffer);
impl LodCutoffBuffer{
    /// System which runs during prepare resources and makes sure a buffer with lod cutoffs is created for scan prepass phase
    pub fn update(
        // All corn fields that may need to update their lod cutoff buffer
        query: Query<(Entity, Option<&Self>, &LodCutoffs, &CornLodInfo), (With<CornField>, Or<(Changed<CornLodInfo>, Changed<LodCutoffs>, Without<Self>)>)>,
        mut commands: Commands,
        render_device: Res<RenderDevice>,
        render_queue: Res<RenderQueue>
    ){
        for (entity, buffer, cutoffs, lod_info) in query.iter(){
            let cutoffs = cutoffs.resize_cutoffs(lod_info.0.len());
            if let Some(buffer) = buffer {
                // If possible, use render queue to write new data
                if buffer.0.size() == cutoffs.len() as u64*4 {
                    buffer.overwrite_cutoffs(&cutoffs, &render_queue);
                    continue;
                }
            }
            commands.entity(entity).insert(Self::new(cutoffs, &render_device));
        }
    }
    /// Writes the new cutoffs into the buffer without making a new buffer
    pub fn overwrite_cutoffs(&self, cutoffs: &Vec<f32>, render_queue: &RenderQueue){
        render_queue.write_buffer(&self.0, 0, bytemuck::cast_slice::<f32, u8>(cutoffs));
    }
    /// Creates a new struct by creating a buffer with the render device
    pub fn new(cutoffs: Vec<f32>, render_device: &RenderDevice) -> Self{
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor{
            label: Some("Corn Field Lod Cutoff Buffer"),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            contents: bytemuck::cast_slice::<f32, u8>(cutoffs.as_slice())
        });
        Self(buffer)
    }
}

#[derive(Debug, Default, Clone)]
pub struct CornCutoffPlugin;
impl Plugin for CornCutoffPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<LodCutoffs>()
            .register_type::<GlobalLodCutoffs>()
            .init_resource::<GlobalLodCutoffs>()
            .add_plugins(ExtractChangedComponentPlugin::<LodCutoffs>::default())
        .sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, GlobalLodCutoffs::extract_to_local)
            .add_systems(Render, LodCutoffBuffer::update.in_set(RenderSet::PrepareResources));
    }
}
