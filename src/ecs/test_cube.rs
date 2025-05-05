use avian3d::prelude::{RigidBody, Collider};
use bevy::{ecs::{component::ComponentId, world::DeferredWorld}, prelude::*};
use lightyear::prelude::{AppComponentExt, NetworkIdentityState, ServerReplicate};
use serde::{Serialize, Deserialize};

/// Test object for debugging network / replication (or whatever)
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, Serialize, Deserialize)]
#[reflect(Component)]
#[component(on_add = TestCube::on_added)]
pub struct TestCube;
impl TestCube {
    fn on_added(mut world: DeferredWorld, entity: Entity, _: ComponentId){
        let assets = world.resource_mut::<AssetServer>();
        let mesh3d = Mesh3d(assets.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))));
        let material = MeshMaterial3d(assets.add(StandardMaterial::from(Color::srgb(1.0, 1.0, 1.0))));
        let net = world.get_resource::<State<NetworkIdentityState>>().map(|s| s.get().clone());
        
        info!("spawning test cube {:?}", net);
        let mut commands = world.commands();
        let mut entity = commands.entity(entity);
        entity.insert((
            Name::new("test cube"),
            mesh3d,
            material,
            Transform::from_translation(Vec3::new(0.0, 2.0, 0.0)),
            RigidBody::Dynamic,
            Collider::cuboid(1.0, 1.0, 1.0),
        ));
        match net {
            Some(NetworkIdentityState::Client) | None => {},
            _ => {
                entity.insert(ServerReplicate::default());
            }
        }
    }
}
impl Plugin for TestCube{
    fn build(&self, app: &mut App) {
        app
            .register_type::<TestCube>()
            .register_component::<TestCube>(lightyear::prelude::ChannelDirection::Bidirectional);
    }
}