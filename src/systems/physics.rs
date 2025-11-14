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

use bevy::ecs::reflect::ReflectBundle;
use wgpu::BlendComponent;

use crate::scenes::resolver::EntityResolver;
#[derive(Debug, Default, Clone, Bundle, Reflect)]
#[reflect(Bundle, Default)]
struct BlenderRigidBody(
    RigidBody,
    DampedPhysics,
    AutoCollider,
);

/// create collider based on all children
#[derive(Debug, Clone, PartialEq, Reflect, Component)]
#[reflect(Component, Default)]
struct AutoCollider(ColliderConstructor);
fn process_autocollider(
    trigger: Trigger<OnAdd, AutoCollider>,
    children: Query<&Children>,
    colliders: Query<&RigidBodyColliders>,
    constructors: Query<&ColliderConstructor>,
){
    // TODO if the RigidBody has colliders or children with collider constructor then skip
    let _ = colliders;

    todo!();

}

impl Default for AutoCollider {
    fn default() -> Self {
        Self(Default::default())
    }
}

/// Marker struct for colliders defined in blender. 
/// empty => 1m cube
/// mesh/object => trimesh
/// assumes parent is rigidbody (inserts static if missing)
#[derive(Debug, Default, Reflect, Component, Serialize, Deserialize)]
#[reflect(Component, Default)]
#[component(storage = "SparseSet")] // I think all temporary effects should be sparse set
pub struct BlenderCollider{
    constructor: Option<ColliderConstructor>
}

impl BlenderCollider {
    fn on_add(
        trigger: Trigger<OnAdd, Self>,
        resolver: EntityResolver,
        meshes: Query<&Mesh3d>,
        rigidbody: Query<&RigidBody>,
        this: Query<&BlenderCollider>,
        mut commands: Commands, 
    ){
        let entity = trigger.target();
        let this = this.get(entity).unwrap();
        
        let rigidbody = resolver.iter_parents_to_scene_root(entity).find(|e|rigidbody.contains(*e));

        // check if entity is mesh, or has child mesh (is object);
        let mut mesh = meshes.contains(entity).then(|| entity);
        if mesh.is_none() {
            mesh = resolver.children.get(entity).ok().and_then(|children| children.iter().find(|c|meshes.contains(*c)));
            if mesh.is_some(){
                warn!("BlenderCollider should be attached to the object {}, not the mesh {}. (handled gracefully)", entity, mesh.unwrap());
            }
        } 
        
        // insert RigidBody::Static on parent if missing
        if rigidbody.is_none(){
            // no parent rigidbody in scene
            let mut parent = match resolver.parents.get(entity) {
                Ok(p) => p.0,
                Err(_) => {
                    warn!(%entity, "no parent in scene for BlenderCollider");
                    entity
                }
            };

            if meshes.contains(entity){
                // actually need to go two up.
                parent = match resolver.parents.get(parent){
                    Ok(p) => p.0,
                    Err(_) => {
                        warn!(%entity, "no parent in scene for BlenderCollider");
                        entity
                    }
                };
            }

            if resolver.scene.contains(parent){
                warn!(entity=%entity, "Scene {} should not be a RigidBody", parent);
            }

            commands.entity(parent).insert_if_new(RigidBody::Static);
        }

        // attach collider constructor
        if mesh.is_some(){
            let constructor = this.constructor.clone().unwrap_or(ColliderConstructor::TrimeshFromMesh);
            // add margin for trimesh (recommended) // EDIT actually adding it for everything.
            // match constructor {
            //     ColliderConstructor::TrimeshFromMesh | ColliderConstructor::TrimeshFromMeshWithConfig(trimesh_flags) => {
            //         commands.entity(entity).insert(CollisionMargin(0.01))
            //     }
            //     _ => {},
            // }
            commands.entity(mesh.unwrap()).insert(constructor);
        } else {
            // is 2.0 because in blender cube defaults to 1.0 half width
            let constructor = this.constructor.clone().unwrap_or(ColliderConstructor::Cuboid { x_length: 2.0, y_length: 2.0, z_length: 2.0 });
            commands.entity(entity).insert(constructor);
        }
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
            .add_plugins(PhysicsPlugins::default().build().disable::<SyncPlugin>() /*handled by lightyear, unfortionately */)
            // .register_type::<ColliderFor>()
            .register_type::<DebugRender>()
            .register_type::<DampedPhysics>()
            .init_resource::<DebugRender>();

        app.add_observer(BlenderCollider::on_add);
        app.register_type::<BlenderCollider>();
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