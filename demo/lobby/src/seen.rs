//! CODEX — GPT-6.1: reusable camera visibility transitions, including scenery occlusion.
use bevy::{
    camera::{primitives::Aabb, CameraUpdateSystems},
    prelude::*,
    picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility},
};
use corn_scripting::ScriptEventsExt;
use crate::controller::Player;

// CODEX: Debounce only disappearance; visible meshes anywhere in the camera count as seen.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct SeenTracker {
    pub seen: bool,
    pub unseen_delay: f32,
    pub max_distance: f32,
    #[reflect(ignore)]
    absent_seconds: f32,
}
// CODEX: Small delay avoids repeated events on a screen edge or between hierarchy updates.
impl Default for SeenTracker {
    fn default() -> Self {
        Self {
            seen: false,
            unseen_delay: 0.15,
            max_distance: 100.0,
            absent_seconds: 0.0,
        }
    }
}
// CODEX: Generic events are available to corn handlers as `on Seen` and `on Unseen`.
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct Seen {
    pub entity: Entity,
}
// CODEX: Emitted only on a seen-to-unseen transition, never every offscreen frame.
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct Unseen {
    pub entity: Entity,
}

// CODEX: Use the final camera pose and projection, independent of the interaction cursor.
pub fn plugin(app: &mut App) {
    app.register_type::<SeenTracker>()
        .register_script_entity_event::<Seen>()
        .register_script_entity_event::<Unseen>()
        .add_systems(
            PostUpdate,
            track
                .after(TransformSystems::Propagate)
                .after(CameraUpdateSystems),
        );
}

// CODEX: Sample each descendant mesh's center and corners; rays prevent seeing through scenery.
fn track(
    mut trackers: Query<(Entity, &mut SeenTracker)>,
    camera: Query<(&Camera, &GlobalTransform), With<Player>>,
    meshes: Query<(&Aabb, &GlobalTransform, Option<&InheritedVisibility>), With<Mesh3d>>,
    children: Query<&Children>,
    time: Res<Time>,
    mut rays: MeshRayCast,
    mut commands: Commands,
) {
    let Some((camera, eye)) = camera.iter().next() else {
        return;
    };
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    for (root, mut tracker) in &mut trackers {
        let parts: Vec<_> = std::iter::once(root)
            .chain(children.iter_descendants(root))
            .collect();
        let visible =
            parts
                .iter()
                .filter_map(|e| meshes.get(*e).ok())
                .any(|(bounds, global, visibility)| {
                    if visibility.is_some_and(|v| !v.get()) {
                        return false;
                    }
                    (0..9).any(|i| {
                        let sign = if i == 8 {
                            Vec3::ZERO
                        } else {
                            Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32)
                                * 2.0
                                - 1.0
                        };
                        let point = global.transform_point(
                            Vec3::from(bounds.center) + Vec3::from(bounds.half_extents) * sign,
                        );
                        let delta = point - eye.translation();
                        if delta.length() > tracker.max_distance {
                            return false;
                        }
                        let Ok(screen) = camera.world_to_viewport(eye, point) else {
                            return false;
                        };
                        if screen.x < 0.0
                            || screen.y < 0.0
                            || screen.x > size.x
                            || screen.y > size.y
                        {
                            return false;
                        }
                        let Ok(direction) = Dir3::new(delta) else {
                            return true;
                        };
                        rays.cast_ray(
                            Ray3d::new(eye.translation(), direction),
                            &MeshRayCastSettings::default()
                                .with_visibility(RayCastVisibility::Visible),
                        )
                        .first()
                        .is_none_or(|(e, hit)| {
                            parts.contains(e) || hit.distance >= delta.length() - 0.002
                        })
                    })
                });
        if visible {
            tracker.absent_seconds = 0.0;
            if !tracker.seen {
                tracker.seen = true;
                commands.trigger(Seen { entity: root });
            }
        } else if tracker.seen {
            tracker.absent_seconds += time.delta_secs();
            if tracker.absent_seconds >= tracker.unseen_delay {
                tracker.seen = false;
                commands.trigger(Unseen { entity: root });
            }
        }
    }
}
