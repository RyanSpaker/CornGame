use avian3d::prelude::{Collider, RigidBody, SleepingDisabled, SleepingThreshold};
use bevy::{ecs::{component::HookContext, world::DeferredWorld}, prelude::*};
use lightyear::prelude::*;
use serde::{Serialize, Deserialize};
use crate::{systems::{network::ReplicateAuto, physics::DampedPhysics}, Headless};

/// Test object for debugging network / replication (or whatever)
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, Serialize, Deserialize)]
#[reflect(Component)]
#[require(
    Name = Name::from("Test Cube"),
    Transform = Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)),
    RigidBody = RigidBody::Dynamic,
    Collider = Collider::cuboid(1.0, 1.0, 1.0),
    DampedPhysics
)]
#[component(on_add = TestCube::add_handles)]
pub struct TestCube;
impl TestCube {
    fn add_handles(mut world: DeferredWorld, HookContext { entity,.. } : HookContext){
        // TODO should not be hook
        if world.get_resource::<Headless>().is_some() {
           return
       }

        let assets = world.resource_mut::<AssetServer>();
        let mesh3d = Mesh3d(assets.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))));
        let material = MeshMaterial3d(assets.add(StandardMaterial::from(Color::srgb(1.0, 1.0, 1.0))));
        
        info!("spawning test cube");
        let mut commands = world.commands();
        let mut entity = commands.entity(entity);
        entity.insert((
            mesh3d,
            material,
            ReplicateAuto,
            SleepingDisabled,
        ));
    }
}
impl Plugin for TestCube{
    fn build(&self, app: &mut App) {
        app
            .register_type::<TestCube>();
    }
}