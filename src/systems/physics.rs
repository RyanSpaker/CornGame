use std::f32::consts::PI;
use bevy::prelude::*;
use avian3d::prelude::*;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
#[require(
    MaxAngularSpeed = MaxAngularSpeed(4.0*PI), 
    MaxLinearSpeed = MaxLinearSpeed(10.0), 
    LinearDamping = LinearDamping(1.0), 
    AngularDamping = AngularDamping(2.0)
)]
pub struct DampedPhysics;

// helper which was used for mesh colliders in blender scenes but isn't actually needed or used.
// #[derive(Debug, Reflect, Serialize, Deserialize)]
// #[reflect(Component)]
// pub enum ColliderFor{
//     Parent
// }
// impl Component for ColliderFor{
//     type Mutability = Mutable;
//     const STORAGE_TYPE: StorageType = StorageType::Table;
//     fn register_component_hooks(hooks: &mut bevy::ecs::component::ComponentHooks) {
//         hooks.on_add(|mut world, hook_context| {
//             let Some(comp) = world.get::<Self>(hook_context.entity) else {return;};
//             info!(entity = %hook_context.entity, component = ?hook_context.component_id, value = ?comp);
//             match comp{
//                 Self::Parent => {
//                     warn!("seriously broken don't use me");
//                     let parent = match world.get::<ChildOf>(hook_context.entity) {Some(p) => p.parent(), None => {return;}};
//                     let Some(mesh) = world.get::<Mesh3d>(hook_context.entity) else {return;};
//                     let Some(meshes) = world.get_resource::<Assets<Mesh>>() else {return;};
//                     let Some(mesh_data) = meshes.get(&mesh.0) else {return;};
//                     let Some(collider) = Collider::trimesh_from_mesh(mesh_data) else {return;};
//                     if let Ok(mut parent) = world.commands().get_entity(parent){
//                         parent.insert((Visibility::Hidden, collider));
//                     }
//                 }
//             }
//         });
//     }
// }

#[derive(Debug, Default, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
struct DebugRender(bool);

pub struct CornPhysicsPlugin;
impl Plugin for CornPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // init physics plugins
        app
            .add_plugins(PhysicsPlugins::default().build().disable::<SyncPlugin>() /*handled by lightyear */)
            .add_plugins(CornPhysicsPluginNetworkPlugin)
            // .register_type::<ColliderFor>()
            .register_type::<DebugRender>()
            .register_type::<DampedPhysics>()
            .init_resource::<DebugRender>();
    }
}

pub struct CornPhysicsPluginNetworkPlugin;
impl Plugin for CornPhysicsPluginNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<LinearVelocity>()
            .add_prediction(PredictionMode::Full);

        app.register_component::<AngularVelocity>()
            .add_prediction(PredictionMode::Full);

        app.register_component::<ExternalForce>()
            .add_prediction(PredictionMode::Full);

        app.register_component::<ExternalImpulse>()
            .add_prediction(PredictionMode::Full);

        // // Do not replicate Transform when we are replicating Position/Rotation!
        // // See https://github.com/cBournhonesque/lightyear/discussions/941
        // // app.register_component::<Transform>()
        // //     .add_prediction(PredictionMode::Full);

        app.register_component::<ComputedMass>()
            .add_prediction(PredictionMode::Full);

        // Position and Rotation have a `correction_fn` set, which is used to smear rollback errors
        // over a few frames, just for the rendering part in postudpate.
        //
        // They also set `interpolation_fn` which is used by the VisualInterpolationPlugin to smooth
        // out rendering between fixedupdate ticks.
        app.register_component::<Position>()
            .add_prediction(PredictionMode::Full)
            .add_linear_interpolation_fn()
            .add_interpolation(InterpolationMode::Full)
            .add_linear_correction_fn();

        app.register_component::<Rotation>()
            .add_prediction(PredictionMode::Full)
            .add_linear_interpolation_fn()
            .add_interpolation(InterpolationMode::Full)
            .add_linear_correction_fn();

        // do not replicate Transform but make sure to register an interpolation function
        // for it so that we can do visual interpolation
        // (another option would be to replicate transform and not use Position/Rotation at all)

        app.register_component::<Transform>()
            .add_interpolation(InterpolationMode::Full)
            .add_interpolation_fn(TransformLinearInterpolation::lerp);
    
    }
}