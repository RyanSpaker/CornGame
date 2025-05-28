use std::f32::consts::PI;
use bevy::prelude::*;
use avian3d::prelude::*;
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
            .add_plugins(PhysicsPlugins::default())
            // .register_type::<ColliderFor>()
            .register_type::<DebugRender>()
            .register_type::<DampedPhysics>()
            .init_resource::<DebugRender>();
    }
}
