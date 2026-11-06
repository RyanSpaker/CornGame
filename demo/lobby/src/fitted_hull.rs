//! CODEX — GPT-6.1: one convex collider from a hierarchy, with named branches excluded.
use avian3d::prelude::*;
use bevy::{math::Affine3A, prelude::*};

// CODEX: Bounds are in the owner's local space and become available when all meshes load.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct FittedHull {
    pub exclude_names: Vec<String>,
    pub center: Vec3,
    pub half_size: Vec3,
}

// CODEX: Register separately from box fitting so exclusions apply to the entire combined hull.
pub fn plugin(app: &mut App) {
    app.register_type::<FittedHull>().add_systems(Update, fit);
}

// CODEX: Walk local transforms; no dependency on last frame's GlobalTransform propagation.
fn fit(
    mut roots: Query<(Entity, &mut FittedHull)>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    transforms: Query<&Transform>,
    names: Query<&Name>,
    meshes: Query<&Mesh3d>,
    assets: Res<Assets<Mesh>>,
    mut commands: Commands,
) {
    for (root, mut hull) in &mut roots {
        if hull.half_size != Vec3::ZERO {
            continue;
        }
        let parts: Vec<_> = children
            .iter_descendants(root)
            .filter(|e| meshes.contains(*e))
            .collect();
        if parts.is_empty() {
            continue;
        }
        let mut points = Vec::new();
        let mut ready = true;
        for part in parts {
            let mut entity = part;
            let mut relative = Affine3A::IDENTITY;
            let mut excluded = false;
            while entity != root {
                if names
                    .get(entity)
                    .is_ok_and(|n| hull.exclude_names.iter().any(|x| x == n.as_str()))
                {
                    excluded = true;
                    break;
                }
                let (Ok(transform), Ok(parent)) = (transforms.get(entity), parents.get(entity))
                else {
                    ready = false;
                    break;
                };
                relative = transform.compute_affine() * relative;
                entity = parent.parent();
            }
            if excluded {
                continue;
            }
            let Some(mesh) = assets.get(&meshes.get(part).unwrap().0) else {
                ready = false;
                break;
            };
            if let Some(vertices) = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .and_then(|p| p.as_float3())
            {
                points.extend(
                    vertices
                        .iter()
                        .map(|p| relative.transform_point3(Vec3::from(*p))),
                );
            }
        }
        if !ready || points.len() < 4 {
            continue;
        }
        let Some(collider) = Collider::convex_hull(&points) else {
            continue;
        };
        let (min, max) = points.iter().fold((Vec3::MAX, Vec3::MIN), |(min, max), p| {
            (min.min(*p), max.max(*p))
        });
        hull.center = (min + max) * 0.5;
        hull.half_size = (max - min) * 0.5;
        commands.entity(root).insert(collider);
    }
}

// CODEX: Verify one hull combines transformed mesh parts while ignoring a named antenna branch.
#[cfg(test)]
mod tests {
    use super::*;
    // CODEX: Includes a nested mesh and a deliberately huge antenna that must not enlarge the collider.
    #[test]
    fn combined_hull_excludes_named_branch() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>().add_systems(Update, fit);
        let body = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(0.2, 0.08, 0.1));
        let antenna = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(0.01, 2.0, 0.01));
        let root = app
            .world_mut()
            .spawn((
                Transform::default(),
                FittedHull {
                    exclude_names: vec!["antenna".into()],
                    ..default()
                },
            ))
            .id();
        app.world_mut()
            .spawn((ChildOf(root), Transform::default(), Mesh3d(body)));
        let branch = app
            .world_mut()
            .spawn((
                Name::new("antenna"),
                ChildOf(root),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ))
            .id();
        app.world_mut()
            .spawn((ChildOf(branch), Transform::default(), Mesh3d(antenna)));
        app.update();
        let hull = app.world().get::<FittedHull>(root).unwrap();
        assert!((hull.half_size - Vec3::new(0.1, 0.04, 0.05)).length() < 0.0001);
        assert!(app.world().get::<Collider>(root).is_some());
        assert_eq!(
            app.world_mut()
                .query::<&Collider>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
