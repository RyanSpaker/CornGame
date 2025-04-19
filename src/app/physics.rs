
use std::f32::consts::PI;

use bevy::prelude::*;
use avian3d::prelude::*;
use serde::{Deserialize, Serialize};

pub struct MyPhysicsPlugin;
impl Plugin for MyPhysicsPlugin {
    fn build(&self, app: &mut App) {
        // init physics plugins
        app.add_plugins((
            PhysicsPlugins::default(),
            // PhysicsDebugPlugin::default(),
            
            //PhysicsDiagnosticsPlugin,
            //PhysicsDiagnosticsUiPlugin,
        ));

        // gltf loading
        app.register_type::<ColliderFor>();
        app.add_systems(PreUpdate, attach_colliders);
        app.add_systems(Update, default_damping);
    }
}

#[derive(Debug, Default, Resource, Reflect, Serialize, Deserialize)]
#[reflect(Resource)]
struct DebugRender(bool);


fn default_damping(
    mut commands: Commands,
    bodies: Query<(Entity, &RigidBody), Added<RigidBody>>
){
    for b in bodies.iter().filter(|b| *b.1 == RigidBody::Dynamic) {
        commands.entity(b.0).insert_if_new((
            MaxAngularSpeed(2.0*PI * 2.0),
            MaxLinearSpeed(10.0),
            LinearDamping(1.0),
            AngularDamping(2.0),
        ));
    }
}


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