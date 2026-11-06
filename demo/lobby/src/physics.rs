//! CLAUDE (claude-opus-5-5): avian for the lobby. As in avian's examples, colliders hang under a rigid
//! body: lobby.corn puts a static one on the scene's root and a dynamic one on the voice box; the
//! colliders (authored there too, only on what needs them) attach to the nearest body above them.
//! corn can't author `RigidBody` itself (immutable components panic when patched through reflection),
//! so it authors [`Body`], which inserts it.
use avian3d::prelude::*;
use bevy::{math::Affine3A, prelude::*};

// CODEX: exercise reflected rigid-body transforms through the actual Avian synchronization systems.
#[cfg(test)]
#[path = "physics_tests.rs"]
mod tests;

pub fn plugin(app: &mut App) {
    app.add_plugins((PhysicsPlugins::default(), PhysicsDebugPlugin))
        .init_resource::<ColliderGizmos>()
        .register_type::<ColliderGizmos>()
        .register_type::<Body>()
        .register_type::<FittedBox>()
        // CLAUDE: corn makes a component from its reflected Default before patching it; avian registers none
        // for Restitution (and its CoefficientCombine field has no Default to build one from)
        .register_type_data::<Restitution, bevy::reflect::std_traits::ReflectDefault>()
        .add_systems(Update, (insert_body, attach_colliders).chain())
        .add_systems(Update, (fit_boxes, show_colliders, warn_negative_scale));
}

/// CLAUDE: avian's collider wireframes, off unless a script turns them on: `resource("ColliderGizmos").on = true`
#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct ColliderGizmos {
    pub on: bool,
}

// CLAUDE: touch the store only to change it: avian resets the visibility of every physics entity's mesh
// whenever the store changes (its hide_meshes option), which would undo e.g. the voice box hiding
fn show_colliders(gizmos: Res<ColliderGizmos>, mut store: ResMut<GizmoConfigStore>) {
    if store.config::<PhysicsGizmos>().0.enabled != gizmos.on {
        store.config_mut::<PhysicsGizmos>().0.enabled = gizmos.on;
    }
}

/// CLAUDE: `Body: Static;` in corn: inserts that `RigidBody`
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct Body(pub RigidBody);

// CLAUDE: a system, not a hook: only in the app's world, not the one corn builds the scene in. On any change,
// so a script patching it (`insert(e, "Body", "Static")`) works too
fn insert_body(mut bodies: Query<(Entity, &Body, Option<&RigidBody>, &mut Transform), Changed<Body>>, mut commands: Commands) {
    for (entity, body, current, mut transform) in &mut bodies {
        if current != Some(&body.0) {
            // CODEX: use the same equivalent sign/rotation decomposition as physics writeback. This
            // preserves mirrored geometry and the imported hierarchy without alternating rotations.
            if transform.scale.min_element() < 0.0 {
                *transform = GlobalTransform::from(*transform).compute_transform();
            }
            commands.entity(entity).insert(body.0);
        }
    }
}

// CODEX: Avian 0.8's hierarchy traversal can skip siblings after a nested body. Attach only missing
// relationships to the nearest existing body; static meshes keep their authored rendering transforms.
fn attach_colliders(
    colliders: Query<Entity, (With<ColliderMarker>, Without<ColliderOf>)>,
    bodies: Query<(), With<RigidBody>>,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    for collider in &colliders {
        if let Some(body) = std::iter::once(collider)
            .chain(parents.iter_ancestors(collider))
            .find(|&entity| bodies.contains(entity))
        {
            commands.entity(collider).insert(ColliderOf { body });
        }
    }
}

// CODEX: report mirrored physics sources once on creation, before a reflected rotation can be mistaken
// for ordinary rigid-body motion. The warning also covers canonicalized mirrored body transforms.
fn warn_negative_scale(
    sources: Query<
        (Entity, Option<&Name>, &Transform),
        Or<(Added<ColliderConstructorHierarchy>, Added<Collider>, Added<RigidBody>)>,
    >,
) {
    for (entity, name, transform) in &sources {
        if transform.scale.min_element() < 0.0 {
            warn!(?entity, ?name, scale = ?transform.scale,
                "Negative scale on a physics source; mirrored rigid bodies require a consistent scale/rotation decomposition. Apply the mirror in the source mesh when possible.");
        }
    }
}

/// CLAUDE: `FittedBox: default;` in corn: one cuboid collider (a child entity) fitted to the vertices of this
/// entity's meshes, axis aligned in its own local space. `center`/`half_size` (local) are filled in once the
/// meshes have loaded, for scripts to read.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct FittedBox {
    pub center: Vec3,
    pub half_size: Vec3,
}

// CLAUDE: local transforms from `mesh` up to `root`, so it doesn't matter whether GlobalTransform has propagated yet
fn relative(mesh: Entity, root: Entity, parents: &Query<&ChildOf>, transforms: &Query<&Transform>) -> Option<Affine3A> {
    let mut affine = Affine3A::IDENTITY;
    let mut e = mesh;
    while e != root {
        affine = transforms.get(e).ok()?.compute_affine() * affine;
        e = parents.get(e).ok()?.parent();
    }
    Some(affine)
}

// CLAUDE
fn fit_boxes(
    mut boxes: Query<(Entity, &mut FittedBox)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    meshes: Query<&Mesh3d>,
    assets: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (entity, mut fitted) in &mut boxes {
        if fitted.half_size != Vec3::ZERO {
            continue;
        }
        let parts: Vec<Entity> = children.iter_descendants(entity).filter(|&e| meshes.contains(e)).collect();
        let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
        let mut ready = !parts.is_empty();
        for &part in &parts {
            let (Some(mesh), Some(to_root)) = (assets.get(&meshes.get(part).unwrap().0), relative(part, entity, &parents, &transforms)) else {
                ready = false;
                break;
            };
            let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()) else {
                continue;
            };
            for p in positions {
                let p = Vec3::from(to_root.transform_point3(Vec3::from(*p)));
                (min, max) = (min.min(p), max.max(p));
            }
        }
        if !ready || min.x > max.x {
            continue;
        }
        fitted.center = (min + max) / 2.0;
        fitted.half_size = (max - min) / 2.0;
        commands.spawn((
            Name::new("fitted box"),
            ChildOf(entity),
            Transform::from_translation(fitted.center),
            Collider::cuboid(max.x - min.x, max.y - min.y, max.z - min.z),
        ));
    }
}
