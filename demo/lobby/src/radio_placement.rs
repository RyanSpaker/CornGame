//! CODEX — GPT-6.1: ray-fit the radio onto its Blender-authored barrel marker once per spawn.
use bevy::{
    prelude::*,
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility},
};
use crate::{fitted_hull::FittedHull, hold::Holdable};
use super::Radio;

// CODEX: Same placement controls as the voice box, without coupling either item's gameplay.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct RadioSpawn {
    pub ray_height: f32,
    pub ray_distance: f32,
    pub gap: f32,
}
// CODEX: The marker sits just above the barrel, so the ray starts beneath nearby overhead geometry.
impl Default for RadioSpawn {
    fn default() -> Self {
        Self {
            ray_height: 0.3,
            ray_distance: 4.0,
            gap: 0.002,
        }
    }
}
// CODEX: Placement never runs again after carrying or dropping the radio.
#[derive(Component)]
struct Placed;
// CODEX: Use the final exported marker pose and loaded combined-hull bounds.
pub fn plugin(app: &mut App) {
    app.register_type::<RadioSpawn>()
        .add_systems(PostUpdate, place.after(TransformSystems::Propagate));
}
// CODEX: Fit all eight hull-bounds corners under the marker's full rotation, preserving authored scale.
fn place(
    markers: Query<(&RadioSpawn, &GlobalTransform)>,
    radios: Query<
        (
            Entity,
            &GlobalTransform,
            &Holdable,
            &FittedHull,
            Option<&ChildOf>,
        ),
        (With<Radio>, Without<Placed>),
    >,
    children: Query<&Children>,
    globals: Query<&GlobalTransform>,
    mut rays: MeshRayCast,
    mut commands: Commands,
) {
    let Some((settings, marker)) = markers.iter().next() else {
        return;
    };
    for (entity, global, holdable, hull, parent) in &radios {
        if hull.half_size == Vec3::ZERO {
            continue;
        }
        let parts: Vec<_> = std::iter::once(entity)
            .chain(children.iter_descendants(entity))
            .collect();
        let origin = marker.translation() + Vec3::Y * settings.ray_height;
        let filter = |e| !parts.contains(&e);
        let Some((_, hit)) = rays
            .cast_ray(
                Ray3d::new(origin, Dir3::NEG_Y),
                &MeshRayCastSettings::default()
                    .with_visibility(RayCastVisibility::Visible)
                    .with_filter(&filter),
            )
            .first()
            .cloned()
        else {
            continue;
        };
        if hit.distance > settings.ray_distance {
            continue;
        }
        let rotation = marker.rotation();
        let scale = global.to_scale_rotation_translation().0;
        let bottom = (0..8)
            .map(|i| {
                let signs = Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32)
                    * 2.0
                    - 1.0;
                (rotation * (scale * (hull.center + hull.half_size * signs))).y
            })
            .fold(f32::INFINITY, f32::min);
        let mut world = Transform {
            translation: marker.translation(),
            rotation,
            scale,
        };
        world.translation.y = origin.y - hit.distance + settings.gap - bottom;
        let local = parent
            .and_then(|p| globals.get(p.parent()).ok())
            .map_or(world, |p| GlobalTransform::from(world).reparented_to(p));
        commands.entity(entity).insert((
            local,
            Visibility::Inherited,
            Placed,
            Holdable {
                home: Some(local),
                ..holdable.clone()
            },
        ));
    }
}
