use std::{marker::PhantomData, hash::Hash};
use bevy::{
    asset::Asset, 
    core_pipeline::{
        core_3d::{AlphaMask3d, Opaque3d, Transmissive3d, Transparent3d},
        deferred::{AlphaMask3dDeferred, Opaque3dDeferred},
        prepass::{AlphaMask3dPrepass, Opaque3dPrepass},
    }, 
    ecs::system::ReadOnlySystemParam, 
    pbr::*, 
    prelude::*, 
    reflect::Reflect, 
    render::{
        render_phase::{DrawFunctions, PhaseItem, RenderCommand, RenderCommandState, SetItemPipeline}, 
        render_resource::AsBindGroup, 
        RenderApp,
    }
};

pub type CustomStandardMaterial = ExtendedMaterial<StandardMaterial, EmptyExtension>;

#[derive(Default, Debug, Clone, AsBindGroup, Asset, Reflect)]
pub struct EmptyExtension{}
impl MaterialExtension for EmptyExtension{
    fn specialize(
        _pipeline: &bevy::pbr::MaterialExtensionPipeline,
        _descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        Ok(())
    }
}

pub type DrawPrepass<M> = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetMaterialBindGroup<M, 2>,
    DrawMesh,
);

pub type DrawMaterial<M> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetMaterialBindGroup<M, 2>,
    DrawMesh,
);

pub type SpecializedDrawPrepass<M, F> = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetMaterialBindGroup<M, 2>,
    F,
);

pub type SpecializedDrawMaterial<M, F> = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetMaterialBindGroup<M, 2>,
    F,
);
/// Adds the ability to add a draw command to the app which pretends to be a draw command of another type
pub trait SpoofRenderCommand{
    fn spoof_render_command<P: PhaseItem, R: RenderCommand<P> + Send + Sync + 'static, L: 'static>(
        &mut self,
    ) -> &mut Self
    where R::Param: ReadOnlySystemParam;
}
impl SpoofRenderCommand for App {
    fn spoof_render_command<P: PhaseItem, R: RenderCommand<P> + Send + Sync + 'static, L: 'static>(
        &mut self,
    ) -> &mut Self
    where
        R::Param: ReadOnlySystemParam,
    {
        let draw_function = RenderCommandState::<P, R>::new(&mut self.world);
        let draw_functions = self
            .world
            .get_resource::<DrawFunctions<P>>()
            .unwrap_or_else(|| {
                panic!(
                    "DrawFunctions<{}> must be added to the world as a resource \
                     before adding render commands to it",
                    std::any::type_name::<P>(),
                );
            });
        draw_functions.write().add_with::<L, _>(draw_function);
        self
    }
}

pub struct SpecializedMaterialPlugin<M: Material, R, P> {
    pub prepass_enabled: bool,
    pub _marker: PhantomData<(M, R, P)>,
}
impl<M: Material, R, P> Default for SpecializedMaterialPlugin<M, R, P> {
    fn default() -> Self {
        Self {
            prepass_enabled: true,
            _marker: Default::default(),
        }
    }
}
impl<M: Material, R, P> Plugin for SpecializedMaterialPlugin<M, R, P>
where
    P: Send + Sync + 'static + RenderCommand<Shadow> +
        RenderCommand<Opaque3dPrepass> + RenderCommand<Opaque3dDeferred> + 
        RenderCommand<AlphaMask3dPrepass> + RenderCommand<AlphaMask3dDeferred>, 
    <P as RenderCommand::<Shadow>>::Param: ReadOnlySystemParam,
    <P as RenderCommand::<Opaque3dPrepass>>::Param: ReadOnlySystemParam,
    <P as RenderCommand::<Opaque3dDeferred>>::Param: ReadOnlySystemParam,
    <P as RenderCommand::<AlphaMask3dPrepass>>::Param: ReadOnlySystemParam,
    <P as RenderCommand::<AlphaMask3dDeferred>>::Param: ReadOnlySystemParam,
    R: Send + Sync + 'static + 
        RenderCommand<Opaque3d> + RenderCommand<Transmissive3d> + RenderCommand<Transparent3d> + RenderCommand<AlphaMask3d>,
    <R as RenderCommand::<Opaque3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand::<AlphaMask3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand::<Transparent3d>>::Param: ReadOnlySystemParam,
    <R as RenderCommand::<Transmissive3d>>::Param: ReadOnlySystemParam,
    M::Data: PartialEq + Eq + Hash + Clone,
{
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<M>::default());
        app.sub_app_mut(RenderApp)
            .spoof_render_command::<Shadow, P, DrawPrepass<M>>()
            .spoof_render_command::<Transmissive3d, R, DrawMaterial<M>>()
            .spoof_render_command::<Transparent3d, R, DrawMaterial<M>>()
            .spoof_render_command::<Opaque3d, R, DrawMaterial<M>>()
            .spoof_render_command::<AlphaMask3d, R, DrawMaterial<M>>()
            .spoof_render_command::<Opaque3dPrepass, P, DrawPrepass<M>>()
            .spoof_render_command::<AlphaMask3dPrepass, P, DrawPrepass<M>>()
            .spoof_render_command::<Opaque3dDeferred, P, DrawPrepass<M>>()
            .spoof_render_command::<AlphaMask3dDeferred, P, DrawPrepass<M>>();
    }
}