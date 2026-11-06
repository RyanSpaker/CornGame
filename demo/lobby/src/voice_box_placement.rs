//! CODEX (GPT-6): Blender's spawn empty sets the voicebox pose; a downward mesh ray sets its height.
use bevy::{
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility},
    prelude::*,
    transform::TransformSystems,
};

use crate::{hold::Holdable, physics::FittedBox, voice_box::VoiceBox};

// CODEX: Authored on the exported empty in lobby.corn; distances and clearance remain tunable there.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct VoiceBoxSpawn {
    pub ray_height: f32,
    pub ray_distance: f32,
    pub gap: f32,
}

// CODEX: Start just above the marker, below the cabin ceiling, and leave a small surface clearance.
impl Default for VoiceBoxSpawn {
    fn default() -> Self {
        Self {
            ray_height: 0.5,
            ray_distance: 4.0,
            gap: 0.002,
        }
    }
}

// CODEX: Each new box is placed once; carrying and dropping must not snap it back to the marker.
#[derive(Component)]
struct Placed;

// CODEX: Run after hierarchy propagation so exported marker rotations and loaded case bounds are current.
pub fn plugin(app: &mut App) {
    app.register_type::<VoiceBoxSpawn>()
        .add_systems(PostUpdate, place.after(TransformSystems::Propagate));
}

// CODEX: Both initial and permission-loss spawns take this path. Wait for the glass case's fitted bounds
// and a real surface hit; keep the marker's full rotation rather than aligning the prop to the surface.
fn place(
    markers: Query<(&GlobalTransform, &VoiceBoxSpawn)>,
    boxes: Query<
        (Entity, &GlobalTransform, &Holdable, Option<&ChildOf>),
        (With<VoiceBox>, Without<Placed>),
    >,
    cases: Query<(&GlobalTransform, &FittedBox)>,
    globals: Query<&GlobalTransform>,
    children: Query<&Children>,
    mut ray_cast: MeshRayCast,
    mut commands: Commands,
) {
    let Some((marker, settings)) = markers.iter().next() else {
        return;
    };
    for (entity, global, holdable, parent) in &boxes {
        let parts: Vec<_> = std::iter::once(entity)
            .chain(children.iter_descendants(entity))
            .collect();
        let Some((case_global, fitted)) = parts
            .iter()
            .filter_map(|&e| cases.get(e).ok())
            .find(|(_, fitted)| fitted.half_size != Vec3::ZERO)
        else {
            continue;
        };
        let origin = marker.translation() + Vec3::Y * settings.ray_height;
        let exclude_self = |e| !parts.contains(&e);
        let Some(hit) = ray_cast
            .cast_ray(
                Ray3d::new(origin, Dir3::NEG_Y),
                &MeshRayCastSettings::default()
                    // CODEX: Placement must also work when the marker's surface is outside the camera view.
                    .with_visibility(RayCastVisibility::Visible)
                    .with_filter(&exclude_self),
            )
            .first()
            .map(|(_, hit)| hit.clone())
        else {
            continue;
        };
        if hit.distance > settings.ray_distance {
            continue;
        }

        // CODEX: Rotate the case's eight corners to find its actual bottom, including tilted markers.
        let rotation = marker.rotation();
        let to_box = global.affine().inverse() * case_global.affine();
        let bottom = (0..8)
            .map(|i| {
                let signs =
                    Vec3::new((i & 1) as f32, (i >> 1 & 1) as f32, (i >> 2 & 1) as f32) * 2.0 - 1.0;
                let corner = to_box.transform_point3(fitted.center + fitted.half_size * signs);
                (rotation * corner).y
            })
            .fold(f32::INFINITY, f32::min);
        let mut world = Transform::from_translation(marker.translation()).with_rotation(rotation);
        world.translation.y = origin.y - hit.distance + settings.gap - bottom;
        let local = parent
            .and_then(|p| globals.get(p.parent()).ok())
            .map_or(world, |p| GlobalTransform::from(world).reparented_to(p));
        commands.entity(entity).insert((
            local,
            Holdable {
                home: Some(local),
                ..holdable.clone()
            },
            Visibility::Inherited,
            Placed,
        ));
    }
}
