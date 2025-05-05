use std::f32::consts::PI;
use bevy::{ecs::component::StorageType, prelude::*};
use avian3d::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
#[reflect(Component)]
#[require(
    MaxAngularSpeed(|| MaxAngularSpeed(4.0*PI)), 
    MaxLinearSpeed(|| MaxLinearSpeed(10.0)), 
    LinearDamping(|| LinearDamping(1.0)), 
    AngularDamping(|| AngularDamping(2.0))
)]
pub struct DampedPhysics;

#[derive(Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub enum ColliderFor{
    Parent
}
impl Component for ColliderFor{
    const STORAGE_TYPE: StorageType = StorageType::Table;
    fn register_component_hooks(hooks: &mut bevy::ecs::component::ComponentHooks) {
        hooks.on_add(|mut world, entity, id| {
            let Some(comp) = world.get::<Self>(entity) else {return;};
            info!("{id:?} added to {entity} with value {comp:?}");
            match comp{
                Self::Parent => {
                    warn!("seriously broken don't use me");
                    let parent = match world.get::<Parent>(entity) {Some(p) => p.get(), None => {return;}};
                    let Some(mesh) = world.get::<Mesh3d>(entity) else {return;};
                    let Some(meshes) = world.get_resource::<Assets<Mesh>>() else {return;};
                    let Some(mesh_data) = meshes.get(&mesh.0) else {return;};
                    let Some(collider) = Collider::trimesh_from_mesh(mesh_data) else {return;};
                    if let Some(mut parent) = world.commands().get_entity(parent){
                        parent.insert((Visibility::Hidden, collider));
                    }
                }
            }
        });
    }
}

#[derive(Debug, Default, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
struct DebugRender(bool);

pub struct CornPhysicsPlugin;
impl Plugin for CornPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // init physics plugins
        app
            .add_plugins(PhysicsPlugins::default())
            .register_type::<ColliderFor>()
            .register_type::<DebugRender>()
            .register_type::<DampedPhysics>()
            .init_resource::<DebugRender>();
    }
}
