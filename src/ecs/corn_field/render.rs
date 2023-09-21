use std::f32::consts::TAU;
use crate::prelude::corn_model::CornMeshes;
use bevy::{
    core_pipeline::core_3d::Opaque3d,
    ecs::system::{lifetimeless::*, SystemParamItem},
    pbr::*,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        mesh::{GpuBufferInfo, MeshVertexBufferLayout},
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult,
            RenderPhase, SetItemPipeline, TrackedRenderPass,
        },
        render_resource::*,
        renderer::RenderDevice,
        view::{ExtractedView, ViewUniform},
        Render, RenderApp, RenderSet, globals::GlobalsUniform, Extract, render_graph::{RenderGraph, Node},
    }, utils::hashbrown::HashMap
};
use bytemuck::{Pod, Zeroable};
use rand::{Rng, distributions::Standard};

//Plugin to enable the rendering of corn_fields as instanced meshes
pub struct CornFieldMaterialPlugin;
impl Plugin for CornFieldMaterialPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_plugins(ExtractComponentPlugin::<CornField>::default())
        .sub_app_mut(RenderApp)
            .add_render_command::<Opaque3d, DrawCorn>()
            .add_render_command::<Shadow, DrawCornShadow>()
            .init_resource::<SpecializedMeshPipelines<CornFieldPipeline>>()
            .init_resource::<SpecializedMeshPipelines<CornShadowPipeline>>()
            .add_systems(
                Render,
                (
                    corn_field_opaque_queue.in_set(RenderSet::Queue),
                    corn_field_shadow_queue.in_set(RenderSet::Queue),
                    prepare_corn_field_buffers.in_set(RenderSet::Prepare)
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CornFieldPipeline>();
        app.sub_app_mut(RenderApp).init_resource::<CornShadowPipeline>();
    }
}

//Function to queue up custom draw sequences for each corn field we have
#[allow(clippy::too_many_arguments)]
fn corn_field_opaque_queue(
    opaque_draw_functions: Res<DrawFunctions<Opaque3d>>,
    corn_fields: Query<(Entity, &CornField), With<CornFieldBuffer>>,
    corn_meshes: Res<CornMeshes>,
    meshes: Res<RenderAssets<Mesh>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Opaque3d>)>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CornFieldPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    corn_pipeline: Res<CornFieldPipeline>,
    msaa: Res<Msaa>
) {
    if !corn_meshes.loaded {return;}
    //Get our custom draw sequence
    let draw_function = opaque_draw_functions.read().id::<DrawCorn>();
    //Rendering setttings from msaa settings
    let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());
    //go through each view, or camera, and add commands to the opaque phase
    for (view, mut opaque_phase) in &mut views {
        //rendering settings from camera
        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        //go through each corn field and queue up draw commands
        for (entity, field) in &corn_fields
        {
            // use the first mesh of our corn model as a refernce for setting up the pipeline.
            let mesh = meshes.get(&corn_meshes.lod_groups[field.lod_level][0].0).unwrap();
            //final rendering settings
            let key =view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);
            //Create the specialized pipeline
            let pipeline = pipelines
                .specialize(&pipeline_cache, &corn_pipeline, key, &mesh.layout)
                .unwrap();
            //Queue up the draw sequence
            opaque_phase.add(Opaque3d { 
                distance: 0.0, 
                pipeline, 
                entity, 
                draw_function
            });
        }
    }
}
//Function to queue up custom draw sequences for each corn field we have To Shadow
#[allow(clippy::too_many_arguments)]
fn corn_field_shadow_queue2(
    opaque_draw_functions: Res<DrawFunctions<Opaque3d>>,
    corn_fields: Query<(Entity, &CornField), With<CornFieldBuffer>>,
    corn_meshes: Res<CornMeshes>,
    meshes: Res<RenderAssets<Mesh>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Shadow>)>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CornFieldPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    corn_pipeline: Res<CornFieldPipeline>,
    msaa: Res<Msaa>
) {
    if !corn_meshes.loaded {return;}
    //Get our custom draw sequence
    let draw_function = opaque_draw_functions.read().id::<DrawCorn>();
    //Rendering setttings from msaa settings
    let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());
    //go through each view, or camera, and add commands to the opaque phase
    for (view, mut shadow_phase) in &mut views {
        //rendering settings from camera
        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        //go through each corn field and queue up draw commands
        for (entity, field) in &corn_fields
        {
            // use the first mesh of our corn model as a refernce for setting up the pipeline.
            let mesh = meshes.get(&corn_meshes.lod_groups[field.lod_level][0].0).unwrap();
            //final rendering settings
            let key =view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);
            //Create the specialized pipeline
            let pipeline = pipelines
                .specialize(&pipeline_cache, &corn_pipeline, key, &mesh.layout)
                .unwrap();
            //Queue up the draw sequence
            shadow_phase.add(Shadow { 
                distance: 0.0, 
                pipeline, 
                entity, 
                draw_function
            });
        }
    }
}


#[allow(clippy::too_many_arguments)]
pub fn corn_field_shadow_queue(
    shadow_draw_functions: Res<DrawFunctions<Shadow>>,
    corn_fields: Query<(Entity, &CornField), With<CornFieldBuffer>>,
    view_lights: Query<(Entity, &ViewLightEntities)>,
    mut view_light_shadow_phases: Query<(&LightEntity, &mut RenderPhase<Shadow>)>,
    corn_meshes: Res<CornMeshes>,
    meshes: Res<RenderAssets<Mesh>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CornShadowPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    corn_pipeline: Res<CornShadowPipeline>
){
    for (entity, view_lights) in &view_lights {
        let draw_shadow_mesh = shadow_draw_functions.read().id::<DrawCornShadow>();
        for view_light_entity in view_lights.lights.iter().copied() {
            let (light_entity, mut shadow_phase) =
                view_light_shadow_phases.get_mut(view_light_entity).unwrap();
            let is_directional_light = matches!(light_entity, LightEntity::Directional { .. });
            //TODO: check to see if light has shadow mapping enabled before qeueing
            for (entity, field) in corn_fields.iter() {
                let mesh = meshes.get(&corn_meshes.lod_groups[field.lod_level][0].0).unwrap();
                let mut mesh_key = 
                    MeshPipelineKey::from_primitive_topology(mesh.primitive_topology)
                    | MeshPipelineKey::DEPTH_PREPASS;
                if is_directional_light {
                    mesh_key |= MeshPipelineKey::DEPTH_CLAMP_ORTHO;
                }

                let pipeline_id = pipelines.specialize(
                    &pipeline_cache,
                    &corn_pipeline,
                    mesh_key,
                    &mesh.layout,
                ).unwrap();

                shadow_phase.add(Shadow {
                    draw_function: draw_shadow_mesh,
                    pipeline: pipeline_id,
                    entity,
                    distance: 0.0
                });
            }
        }
    }
}



//component attached to the render world corn fields that holds the byte buffer of data for the rendering
#[derive(Component, Debug)]
pub struct CornFieldBuffer {
    buffer: Buffer,
    length: usize,
}

//creates the byte buffer and attaches it to corn field entities
fn prepare_corn_field_buffers(
    mut commands: Commands,
    query: Query<(Entity, &CornField)>,
    render_device: Res<RenderDevice>,
) {
    for (entity, instance_data) in &query {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("corn field data buffer"),
            contents: bytemuck::cast_slice(instance_data.instance_data.as_slice()),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });
        commands.entity(entity).insert(CornFieldBuffer {
            buffer,
            length: instance_data.instance_data.len(),
        });
    }
}

// Custom Pipeline for corn instanced rendering, where we define stuff like shader constants and bind groups
//Also where we specify the shader to use
#[derive(Resource)]
pub struct CornFieldPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline
}
impl FromWorld for CornFieldPipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let shader = asset_server.load("shaders/instancing.wgsl");
        let mesh_pipeline = world.resource::<MeshPipeline>();
        CornFieldPipeline {
            shader,
            mesh_pipeline: mesh_pipeline.clone()
        }
    }
}
impl SpecializedMeshPipeline for CornFieldPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        //get descriptor from mesh data
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;
        // meshes typically live in bind group 2. because we are using bindgroup 1
        // we need to add MESH_BINDGROUP_1 shader def so that the bindings are correctly
        // linked in the shader
        descriptor
            .vertex
            .shader_defs
            .push("MESH_BINDGROUP_1".into());
        //change our vertex attributes
        descriptor.vertex.buffers = vec![
            layout.get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)]).unwrap(),
            VertexBufferLayout {
                array_stride: std::mem::size_of::<PerCornData>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 1,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: VertexFormat::Float32x3.size(),
                        shader_location: 2,
                    },
                ],
            }
        ];
        //set our shader
        descriptor.vertex.shader = self.shader.clone();
        descriptor.fragment.as_mut().map(|f| {f.shader = self.shader.clone(); return f;});
        Ok(descriptor)
    }
}
//Custom pipeline for drawing corn shadows
#[derive(Resource)]
pub struct CornShadowPipeline {
    pub view_layout_no_motion_vectors: BindGroupLayout,
    pub mesh_layouts: MeshLayouts,
    pub material_vertex_shader: Option<Handle<Shader>>,
    pub material_fragment_shader: Option<Handle<Shader>>
}
impl FromWorld for CornShadowPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();
        let asset_server = world.resource::<AssetServer>();

        let view_layout_no_motion_vectors =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                entries: &[
                    // View
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: Some(ViewUniform::min_size()),
                        },
                        count: None,
                    },
                    // Globals
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::VERTEX_FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(GlobalsUniform::min_size()),
                        },
                        count: None,
                    },
                ],
                label: Some("prepass_view_layout_no_motion_vectors"),
            });

        let mesh_pipeline = world.resource::<MeshPipeline>();

        CornShadowPipeline {
            view_layout_no_motion_vectors,
            mesh_layouts: mesh_pipeline.mesh_layouts.clone(),
            material_vertex_shader: Some(asset_server.load("shaders/instancing.wgsl")),
            material_fragment_shader: Some(asset_server.load("shaders/instancing.wgsl")),
        }
    }
}
impl SpecializedMeshPipeline for CornShadowPipeline{
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut bind_group_layouts = vec![self.view_layout_no_motion_vectors.clone(), self.mesh_layouts.model_only.clone()];
        let mut shader_defs = Vec::new();
        let mut vertex_attributes = Vec::new();
        shader_defs.push("DEPTH_PREPASS".into());
        shader_defs.push("VERTEX_POSITIONS".into());
        vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));

        if key.contains(MeshPipelineKey::DEPTH_CLAMP_ORTHO) {
            shader_defs.push("DEPTH_CLAMP_ORTHO".into());
            shader_defs.push("PREPASS_FRAGMENT".into());
        }

        let vertex_buffer_layout = layout.get_layout(&vertex_attributes)?;

        // The fragment shader is only used when the normal prepass or motion vectors prepass
        // is enabled or the material uses alpha cutoff values and doesn't rely on the standard
        // prepass shader or we are clamping the orthographic depth.
        let fragment_required = key.contains(MeshPipelineKey::DEPTH_CLAMP_ORTHO)
            && self.material_fragment_shader.is_some();

        let fragment = fragment_required.then(|| {
            // Use the fragment shader from the material
            let frag_shader_handle = self.material_fragment_shader.clone().unwrap();

            FragmentState {
                shader: frag_shader_handle,
                entry_point: "fragment".into(),
                shader_defs: shader_defs.clone(),
                targets: vec![],
            }
        });

        // Use the vertex shader from the material if present
        let vert_shader_handle = self.material_vertex_shader.clone().unwrap();

        let mut push_constant_ranges = Vec::with_capacity(1);
        if cfg!(all(feature = "webgl", target_arch = "wasm32")) {
            push_constant_ranges.push(PushConstantRange {
                stages: ShaderStages::VERTEX,
                range: 0..4,
            });
        }

        let mut descriptor = RenderPipelineDescriptor {
            vertex: VertexState {
                shader: vert_shader_handle,
                entry_point: "vertex".into(),
                shader_defs,
                buffers: vec![vertex_buffer_layout],
            },
            fragment,
            layout: bind_group_layouts,
            primitive: PrimitiveState {
                topology: key.primitive_topology(),
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            push_constant_ranges,
            label: Some("prepass_pipeline".into()),
        };
        // meshes typically live in bind group 2. because we are using bindgroup 1
        // we need to add MESH_BINDGROUP_1 shader def so that the bindings are correctly
        // linked in the shader
        descriptor
            .vertex
            .shader_defs
            .push("MESH_BINDGROUP_1".into());
        //change our vertex attributes
        descriptor.vertex.buffers = vec![
            layout.get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)]).unwrap(),
            VertexBufferLayout {
                array_stride: std::mem::size_of::<PerCornData>() as u64,
                step_mode: VertexStepMode::Instance,
                attributes: vec![
                    VertexAttribute {
                        format: VertexFormat::Float32x3,
                        offset: 0,
                        shader_location: 1,
                    },
                    VertexAttribute {
                        format: VertexFormat::Float32x2,
                        offset: VertexFormat::Float32x3.size(),
                        shader_location: 2,
                    },
                ],
            }
        ];
        Ok(descriptor)
    }
}
//Custom function sequence for drawing instanced corn
type DrawCorn = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    DrawCornInstanced,
);
//Custom Shadow draw sequence
type DrawCornShadow = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetMeshBindGroup<1>,
    DrawCornInstanced
);
//Draw Function for instanced corn
pub struct DrawCornInstanced;
impl<P: PhaseItem> RenderCommand<P> for DrawCornInstanced {
    type Param = (SRes<RenderAssets<Mesh>>, SRes<CornMeshes>);
    type ViewWorldQuery = ();
    type ItemWorldQuery = (Read<CornField>, Read<CornFieldBuffer>);

    #[inline]
    fn render<'w>(
        _item: &P,
        _view: (),
        (corn_field, corn_buffer): (&'w CornField, &'w CornFieldBuffer),
        meshes: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        pass.set_vertex_buffer(1, corn_buffer.buffer.slice(..));
        let mesh_assets = meshes.0.into_inner();
        for gpu_mesh in meshes.1.into_inner().lod_groups[corn_field.lod_level].iter().filter_map(|m| mesh_assets.get(&m.0)){
            pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            match &gpu_mesh.buffer_info {
                GpuBufferInfo::Indexed {
                    buffer,
                    index_format,
                    count,
                } => {
                    pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                    pass.draw_indexed(0..*count, 0, 0..corn_buffer.length as u32);
                }
                GpuBufferInfo::NonIndexed => {
                    pass.draw(0..gpu_mesh.vertex_count, 0..corn_buffer.length as u32);
                }
            }
        }
        RenderCommandResult::Success
    }
}
