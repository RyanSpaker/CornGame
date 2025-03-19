
use bevy::prelude::*;
use avian3d::prelude::*;
use serde::{Deserialize, Serialize};

pub struct MyPhysicsPlugin;
impl Plugin for MyPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // init physics plugins
        app.add_plugins((
            PhysicsPlugins::default(),
            PhysicsDebugPlugin::default(),
            //PhysicsDiagnosticsPlugin,
            //PhysicsDiagnosticsUiPlugin,
        ));

        // gltf loading
        app.register_type::<ColliderFor>();
        app.add_systems(PreUpdate, attach_colliders);
    }
}

#[derive(Debug, Default, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
struct DebugRender(bool);


#[derive(Debug, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub enum ColliderFor{
    Parent
}

fn attach_colliders(
    mut commands: Commands,
    world: &World, 
    asset: Res<Assets<Mesh>>,
    mut items: Query<(Entity, &Mesh3d, &ColliderFor), Added<ColliderFor>>
){
    for (id, mesh, item) in &mut items {
        let e = world.entity(id);
        match item {
            ColliderFor::Parent => {
                warn!("seriously broken don't use me");
                let p = e.get::<Parent>();
                let mesh = asset.get(&mesh.0).unwrap();
                commands.entity(p.unwrap().get())
                    .insert(Visibility::Hidden)
                    .insert(Collider::trimesh_from_mesh(mesh).unwrap());
            }
        }
    }  
}