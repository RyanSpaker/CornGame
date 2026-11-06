//! CLAUDE (claude-opus-5-5): carrying things. E on a [`Holdable`] picks it up (one at a time). While
//! [`Held`] it floats in front of the camera, below the cursor, sized to a share of the screen height,
//! with its rotation fixed in the world, so looking around shows its other sides. R drops a droppable
//! one onto the flat surface behind it (if what's behind it isn't flat: from where it is), as an avian
//! rigid body. Scripts take [`Held`] off to put things elsewhere
//! (the hook, the door's key slot).
//! CODEX (GPT-6): configurable collider-centered carry depth and camera-relative prop aiming.
//! CODEX (GPT-6.1): optional pickup rotation keeps bounds anchored to their original orientation.
use avian3d::prelude::*;
use bevy::{camera::primitives::Aabb, prelude::*};
use corn_scripting::ScriptEventsExt;

use crate::{
    controller::{Player, PlayerBody},
    interaction::Interact,
};

pub const DROP_KEY: KeyCode = KeyCode::KeyR;
// CODEX (GPT-6): secondary held-item input is independent of E's world interaction targeting.
pub const TOGGLE_KEY: KeyCode = KeyCode::KeyF;
/// CLAUDE: how far the drop ray looks for a surface
const DROP_RANGE: f32 = 6.0;
/// CLAUDE: gap left between the dropped item and the surface
const DROP_GAP: f32 = 0.02;
/// CLAUDE: the dropped item is tilted from how it was held (about a horizontal axis) by this much, radians:
/// always some, never much; and spun about the vertical by up to DROP_SPIN either way
const DROP_TILT: std::ops::Range<f32> = 0.1..0.3;
const DROP_SPIN: f32 = 0.3;
/// CLAUDE: a surface the drop ray hits is put on only if this flat: the height of its averaged normal, at
/// most a 15% incline (1 / sqrt(1 + 0.15²))
const DROP_ONTO: f32 = 0.9889;
/// CLAUDE: the surface is sampled by parallel rays this share of the item's radius around the centre one
const DROP_SAMPLES: f32 = 0.6;

// CLAUDE: authored in lobby.corn
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Holdable {
    /// baseline screen-height share before depth_scale brings the item closer
    pub screen_height: f32,
    /// where its centre is held: this share of the screen height below the screen's centre
    pub below: f32,
    /// R drops it (with physics)
    pub droppable: bool,
    /// while held and aiming at nothing: "press E to {use_prompt}", E triggers `Interact` on it. empty: none
    pub use_prompt: String,
    /// CODEX: optional F action, shown even when E targets something else; empty disables F.
    pub toggle_prompt: String,
    /// its Transform where it was authored, set when it spawns (scripts put things back there)
    pub home: Option<Transform>,
    /// CODEX: paper keeps facing the view; ordinary props retain their world rotation.
    pub face_camera: bool,
    /// CODEX: multiplier on the screen-size-derived distance; override per prop in .corn.
    pub depth_scale: f32,
    /// CODEX: use collider bounds for carrying/drop geometry; mesh bounds are the fallback.
    pub center_on_collider: bool,
    /// CODEX: optional hand-authored carry center in this entity's local space (zero uses its origin).
    pub center_offset: Option<Vec3>,
    /// CODEX: nonzero local forward axis aims away from the player, with no camera roll.
    pub aim_forward: Vec3,
    /// CODEX: camera pitch multiplier for aimed props, e.g. 0.5 gives 45 degrees at a 90-degree look.
    pub aim_pitch: f32,
}

impl Default for Holdable {
    fn default() -> Self {
        Self {
            screen_height: 0.2, below: 0.05, droppable: false, use_prompt: String::new(), toggle_prompt: String::new(), home: None,
            face_camera: false, depth_scale: 0.5, center_on_collider: true, center_offset: None,
            aim_forward: Vec3::ZERO, aim_pitch: 0.5,
        }
    }
}

/// CLAUDE: being carried. Measured when picked up: the world rotation it keeps, and its bounds relative
/// to its origin (constant while the rotation is).
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
#[component(storage = "SparseSet")]
pub struct Held {
    /// its own Transform when it was picked up, for putting it back
    pub from: Transform,
    pub rotation: Quat,
    pub scale: Vec3,
    /// bounding sphere centre, from the origin, in world axes
    pub center: Vec3,
    pub radius: f32,
    /// collider (or fallback mesh) bounds' corners, from the origin, in world axes
    #[reflect(ignore)]
    pub corners: Vec<Vec3>,
}

// CODEX: Shared pause before any held rotation correction begins; tune independently of duration.
pub(crate) const ROTATION_DELAY_SECONDS: f32 = 0.2;

// CODEX: A one-shot world-space correction; keep the target after finishing until Held is removed.
#[derive(Component)]
pub(crate) struct HeldRotation {
    /// CODEX: Retargeting starts at the current pose while cached carry bounds keep their basis.
    pub from: Option<Quat>,
    pub target: Quat,
    pub started_at: f64,
    pub seconds: f32,
}

// CODEX: Sampling in the shared carry/drop pose avoids system-order dependencies and release snaps.
impl HeldRotation {
    // CODEX: Original pickup rotation remains the coordinate basis for the cached center and corners.
    pub(crate) fn sample(&self, from: Quat, now: f64) -> Quat {
        let t = (((now - self.started_at) as f32 - ROTATION_DELAY_SECONDS) / self.seconds.max(0.001)).clamp(0.0, 1.0);
        // CODEX: Quintic smootherstep has zero speed and acceleration at both ends.
        let eased = t * t * t * (t * (6.0 * t - 15.0) + 10.0);
        self.from.unwrap_or(from).slerp(self.target, eased)
    }
}

// CODEX (GPT-6): corn handles the item's switch without conflating it with pickup's Interact event.
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct ToggleHeld {
    pub entity: Entity,
}

pub fn plugin(app: &mut App) {
    app.register_type::<Holdable>()
        .register_type::<Held>()
        .register_script_entity_event::<ToggleHeld>()
        .add_observer(pick_up)
        .add_observer(|add: On<Add<Holdable>>, mut items: Query<(&mut Holdable, &Transform)>| {
            if let Ok((mut holdable, transform)) = items.get_mut(add.entity) {
                holdable.home.get_or_insert(*transform);
            }
        })
        .add_observer(on_held)
        .add_observer(on_released)
        .add_systems(Update, (drop, toggle, carry).chain().after(crate::controller::walk));
}

// CODEX: F only reaches a held item during gameplay; dropping keeps the item's switch state intact.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<crate::computer::ComputerSession>,
    paper: Res<crate::post_it::PostItSession>,
    items: Query<(Entity, &Holdable), With<Held>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(TOGGLE_KEY) || !crate::browser::captured()
        || session.active.is_some() || session.returning || paper.blocks_input()
    {
        return;
    }
    for (entity, holdable) in &items {
        if !holdable.toggle_prompt.is_empty() {
            commands.trigger(ToggleHeld { entity });
        }
    }
}

// CLAUDE
fn pick_up(
    on: On<Interact>,
    holdables: Query<&Holdable, Without<Held>>,
    held: Query<(), With<Held>>,
    globals: Query<(&GlobalTransform, &Transform)>,
    children: Query<&Children>,
    meshes: Query<(&Aabb, &GlobalTransform)>,
    colliders: Query<(&Collider, &GlobalTransform)>,
    mut commands: Commands,
) {
    let Ok(holdable) = holdables.get(on.entity) else {
        return;
    };
    if !held.is_empty() {
        return;
    }
    let Ok((global, &from)) = globals.get(on.entity) else {
        return;
    };
    let (scale, rotation, origin) = global.to_scale_rotation_translation();
    // CODEX: collider.shape() is unscaled; GlobalTransform supplies scale exactly once, including
    // fitted child-collider translations. Never include a sibling prop's collider in these bounds.
    let mut corners: Vec<Vec3> = std::iter::once(on.entity)
        .chain(children.iter_descendants(on.entity))
        .filter_map(|e| colliders.get(e).ok())
        .filter(|_| holdable.center_on_collider)
        .flat_map(|(collider, global)| {
            let aabb = collider.shape().compute_local_aabb();
            bounds_corners(aabb.center().into(), aabb.half_extents().into(), global, origin)
        })
        .collect();
    if corners.is_empty() {
        corners = std::iter::once(on.entity)
            .chain(children.iter_descendants(on.entity))
            .filter_map(|e| meshes.get(e).ok())
            .flat_map(|(aabb, mesh)| bounds_corners(aabb.center.into(), aabb.half_extents.into(), mesh, origin))
            .collect();
    }
    let (min, max) = corners
        .iter()
        .fold((Vec3::MAX, Vec3::MIN), |(min, max), &c| (min.min(c), max.max(c)));
    let (center, radius) = if corners.is_empty() {
        (Vec3::ZERO, 0.1)
    } else {
        ((min + max) / 2.0, ((max - min) / 2.0).length())
    };
    let center = holdable.center_offset.map_or(center, |local| global.transform_point(local) - origin);
    commands.entity(on.entity).insert(Held { from, rotation, scale, center, radius, corners });
}

// CODEX: transform all eight bounds corners, preserving child offsets and nonuniform scale.
fn bounds_corners(center: Vec3, half: Vec3, global: &GlobalTransform, origin: Vec3) -> [Vec3; 8] {
    std::array::from_fn(|i| {
        let sign = Vec3::new((i & 1) as f32, (i >> 1 & 1) as f32, (i >> 2 & 1) as f32) * 2.0 - 1.0;
        global.transform_point(center + half * sign) - origin
    })
}

/// CLAUDE: its colliders (if any) leave the world while it's carried; its body (if any, authored static
/// where it sits) follows the hand as kinematic, and is dynamic once dropped
fn on_held(
    add: On<Add<Held>>,
    children: Query<&Children>,
    colliders: Query<(), With<Collider>>,
    bodies: Query<(), With<RigidBody>>,
    mut commands: Commands,
) {
    // with no velocity left over from falling: a kinematic body keeps moving by it (once let go, before
    // whoever let go of it sets its body)
    if bodies.contains(add.entity) {
        commands.entity(add.entity).insert((RigidBody::Kinematic, LinearVelocity::ZERO, AngularVelocity::ZERO));
    }
    for e in std::iter::once(add.entity).chain(children.iter_descendants(add.entity)) {
        if colliders.contains(e) {
            commands.entity(e).insert(ColliderDisabled);
        }
    }
}

/// CLAUDE: and come back when it's put down, wherever that is (whoever puts it down sets the body)
fn on_released(remove: On<Remove<Held>>, children: Query<&Children>, colliders: Query<(), With<Collider>>, mut commands: Commands) {
    // CODEX: A later pickup must evaluate its new tilt and never reuse an earlier correction.
    commands.entity(remove.entity).try_remove::<HeldRotation>();
    for e in std::iter::once(remove.entity).chain(children.iter_descendants(remove.entity)) {
        if colliders.contains(e) {
            commands.entity(e).try_remove::<ColliderDisabled>();
        }
    }
}

/// CLAUDE: the camera's world pose and the view-space direction of a point `below` screen heights under
/// the screen's centre, with tan(fov / 2)
fn view(camera: &Camera, global: &GlobalTransform, below: f32) -> (Vec3, Dir3, f32) {
    let clip = camera.clip_from_view();
    let direction = clip
        .inverse()
        .project_point3(Vec3::new(0.0, -2.0 * below, 0.5))
        .try_normalize()
        .filter(|d| d.z < 0.0)
        .unwrap_or(Vec3::NEG_Z);
    let tan_half_fov = 1.0 / clip.y_axis.y.max(1e-3);
    let world = Dir3::new(global.rotation() * direction).unwrap_or(Dir3::NEG_Z);
    (global.translation(), world, tan_half_fov)
}

// CODEX: one pose calculation for carrying and dropping, so aimed props do not snap back on R.
fn held_pose(held: &Held, holdable: &Holdable, camera: &Camera, global: &GlobalTransform, correction: Option<&HeldRotation>, now: f64) -> Transform {
    let (eye, direction, tan_half_fov) = view(camera, global, holdable.below);
    let distance = held.radius / (holdable.screen_height * tan_half_fov).max(1e-3) * holdable.depth_scale.max(0.01);
    let rotation = if holdable.face_camera {
        // CODEX: local +Y is the authored paper front, local -Z its top edge.
        global.rotation() * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)
    } else if let Some(forward) = holdable.aim_forward.try_normalize() {
        let (yaw, pitch, _) = global.rotation().to_euler(EulerRot::YXZ);
        Quat::from_euler(EulerRot::YXZ, yaw, pitch * holdable.aim_pitch, 0.0)
            * Quat::from_rotation_arc(forward, Vec3::NEG_Z)
    } else {
        correction.map_or(held.rotation, |tween| tween.sample(held.rotation, now))
    };
    let center = rotation * held.rotation.inverse() * held.center;
    Transform { translation: eye + direction * distance - center, rotation, scale: held.scale }
}

/// CLAUDE: world pose -> the entity's own Transform under its parent
fn local(world: Transform, parent: Option<&GlobalTransform>) -> Transform {
    match parent {
        Some(parent) => GlobalTransform::from(world).reparented_to(parent),
        None => world,
    }
}

/// CLAUDE: keep it in front of the camera. Runs after walking, before transforms propagate.
pub(crate) fn carry(
    time: Res<Time>,
    cameras: Query<(&Camera, &Transform, Option<&ChildOf>), With<Player>>,
    mut items: Query<(&Held, &Holdable, &mut Transform, Option<&ChildOf>, Option<&HeldRotation>), Without<Player>>,
    globals: Query<&GlobalTransform>,
) {
    let Some((camera, eye, eye_parent)) = cameras.iter().next() else {
        return;
    };
    let eye = eye_parent
        .and_then(|p| globals.get(p.parent()).ok())
        .map_or(GlobalTransform::from(*eye), |p| p.mul_transform(*eye));
    for (held, holdable, mut transform, parent, correction) in &mut items {
        let world = held_pose(held, holdable, camera, &eye, correction, time.elapsed_secs_f64());
        let parent = parent.and_then(|p| globals.get(p.parent()).ok());
        *transform = local(world, parent);
    }
}

/// CLAUDE: R: cast from the camera through the held item's centre; put it just above what that hits, a
/// little turned, and let it fall.
fn drop(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    session: Res<crate::computer::ComputerSession>,
    spatial: SpatialQuery,
    cameras: Query<(&Camera, &Transform, Option<&ChildOf>), With<Player>>,
    items: Query<(Entity, &Held, &Holdable, Option<&ChildOf>, Option<&HeldRotation>)>,
    globals: Query<&GlobalTransform>,
    children: Query<&Children>,
    player_bodies: Query<Entity, With<PlayerBody>>,
    mut commands: Commands,
    tuning: Option<Res<crate::radio::RadioTuning>>,
) {
    if !keys.just_pressed(DROP_KEY) || !crate::browser::captured() || session.active.is_some() {
        return;
    }
    // CODEX: Finish tuning before releasing an item, so mouse capture cannot outlive its placement.
    if tuning.is_some_and(|t| t.entity.is_some()) { return; }
    let Some((camera, eye, eye_parent)) = cameras.iter().next() else {
        return;
    };
    let eye = eye_parent
        .and_then(|p| globals.get(p.parent()).ok())
        .map_or(GlobalTransform::from(*eye), |p| p.mul_transform(*eye));
    for (entity, held, holdable, parent, correction) in &items {
        if !holdable.droppable {
            continue;
        }
        let pose = held_pose(held, holdable, camera, &eye, correction, time.elapsed_secs_f64());
        let aimed = pose.rotation * held.rotation.inverse();
        let carried_center = aimed * held.center;
        let start = eye.translation();
        let Ok(direction) = Dir3::new(pose.translation + carried_center - start) else {
            continue;
        };
        // a small turn about a random axis, from a hash of the time
        let seed = (time.elapsed_secs_f64() * 1e4) as u64;
        let random = |k: u64| {
            let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(k.wrapping_mul(0xBF58_476D_1CE4_E5B9));
            x ^= x >> 31;
            x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^= x >> 29;
            (x >> 11) as f32 / (1u64 << 53) as f32
        };
        let heading = random(1) * std::f32::consts::TAU;
        let tilt = DROP_TILT.start + random(2) * (DROP_TILT.end - DROP_TILT.start);
        let turn = Quat::from_axis_angle(Vec3::new(heading.cos(), 0.0, heading.sin()), tilt)
            * Quat::from_rotation_y((random(3) * 2.0 - 1.0) * DROP_SPIN);
        let rotation = turn * pose.rotation;

        // not itself (its colliders are off anyway), not the player's body the ray starts in
        let parts: Vec<Entity> = std::iter::once(entity)
            .chain(children.iter_descendants(entity))
            .chain(player_bodies.iter())
            .collect();
        let filter = SpatialQueryFilter::from_excluded_entities(parts);
        let hit = spatial.cast_ray(start, direction, DROP_RANGE, true, &filter);
        // onto a floor, table or the like; a wall (or nothing) behind it: let go where it is. Walls are logs,
        // some with tops facing up: the normals of the centre ray's hit and of 4 rays around it, averaged
        let flat = hit.filter(|_| {
            let u = direction.any_orthonormal_vector();
            let v = direction.cross(u);
            let offsets = [Vec3::ZERO, u, -u, v, -v].map(|o| o * held.radius * DROP_SAMPLES);
            let normals: Vec3 = offsets
                .iter()
                .filter_map(|&o| spatial.cast_ray(start + o, direction, DROP_RANGE, true, &filter))
                .map(|h| h.normal.normalize_or_zero())
                .sum();
            normals.normalize_or_zero().y >= DROP_ONTO
        });
        let translation = match flat {
            Some(hit) => {
                let point = start + direction * hit.distance;
                let normal = hit.normal.normalize();
                // how far the turned item reaches past its origin toward the surface
                let reach = held
                    .corners
                    .iter()
                    .map(|&c| (turn * aimed * c).dot(-normal))
                    .fold(0.0, f32::max);
                // and its centre over the hit point
                let center = turn * carried_center;
                point + normal * (reach + DROP_GAP) - center.reject_from_normalized(normal)
            }
            // CODEX: turning about the selected carry center must not shift it when released in air.
            None => pose.translation + carried_center - turn * carried_center,
        };
        let world = Transform { translation, rotation, scale: held.scale };
        let parent = parent.and_then(|p| globals.get(p.parent()).ok());
        commands
            .entity(entity)
            .remove::<Held>()
            .insert((local(world, parent), RigidBody::Dynamic, LinearVelocity::ZERO, AngularVelocity::ZERO));
    }
}

// CLAUDE: avian with this crate's features: a dynamic box dropped on a plane rests on it
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{CameraProjection, ComputedCameraValues};
    use bevy::time::TimeUpdateStrategy;

    // CODEX: Both carrying and dropping call this pose path; rotation must preserve the carry center.
    #[test]
    fn pickup_rotation_eases_about_carry_center_and_clears_on_release() {
        let original = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let local_center = Vec3::new(0.15, -0.03, 0.06);
        let held = Held { rotation: original, scale: Vec3::ONE,
            center: original * local_center, radius: 0.2, ..default() };
        let correction = HeldRotation { from: None, target: Quat::from_rotation_y(0.7), started_at: 1.0, seconds: 0.7 };
        let camera = Camera { computed: ComputedCameraValues {
            clip_from_view: PerspectiveProjection::default().get_clip_from_view(), ..default()
        }, ..default() };
        let eye = GlobalTransform::from(Transform::from_xyz(0.0, 1.6, 0.0));
        let holdable = Holdable::default();
        let total_angle = original.angle_between(correction.target);
        let mut previous_angle = 0.0;
        for t in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0, 2.0] {
            let pose = held_pose(&held, &holdable, &camera, &eye, Some(&correction), 1.0 + ROTATION_DELAY_SECONDS as f64 + t * 0.7);
            let (position, direction, tan_half_fov) = view(&camera, &eye, holdable.below);
            let distance = held.radius / (holdable.screen_height * tan_half_fov) * holdable.depth_scale;
            assert!(pose.transform_point(local_center).distance(position + direction * distance) < 1e-5,
                "rotation must pivot around the carry center, t={t}");
            let angle = original.angle_between(pose.rotation);
            assert!(angle + 1e-5 >= previous_angle, "turn must progress without overshoot");
            if t == 0.0 { assert!(angle < 1e-3, "pickup begins in the original pose"); }
            if t == 0.25 { assert!(angle < total_angle * 0.2, "deliberate start"); }
            if t == 0.75 { assert!(angle > total_angle * 0.8, "gentle settling"); }
            if t >= 1.0 { assert!(pose.rotation.angle_between(correction.target) < 1e-3); }
            previous_angle = angle;
        }
        let mut app = App::new();
        app.add_observer(on_released);
        let item = app.world_mut().spawn((held, correction)).id();
        app.world_mut().entity_mut(item).remove::<Held>();
        assert!(app.world().get::<HeldRotation>(item).is_none(), "release clears the one-shot target");
    }

    // CODEX: off-origin child colliders take precedence over visible mesh bounds, with local overrides.
    #[test]
    fn pickup_centers_on_collider_with_per_object_override() {
        for (collider_center, override_center, expected) in [
            (true, None, Vec3::new(0.3, -0.2, 0.1)),
            (false, None, Vec3::new(-0.5, 0.4, 0.2)),
            (true, Some(Vec3::ZERO), Vec3::ZERO),
            (true, Some(Vec3::new(0.1, 0.2, 0.3)), Vec3::new(0.1, 0.2, 0.3)),
        ] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, TransformPlugin)).add_observer(pick_up);
            let from = Transform::from_xyz(2.0, 1.0, -3.0)
                .with_rotation(Quat::from_rotation_z(0.7)).with_scale(Vec3::new(2.0, 3.0, 4.0));
            let item = app.world_mut().spawn((from, Holdable {
                center_on_collider: collider_center, center_offset: override_center, ..default()
            })).id();
            app.world_mut().spawn((ChildOf(item), Transform::from_xyz(0.3, -0.2, 0.1), Collider::cuboid(0.2, 0.3, 0.4)));
            app.world_mut().spawn((ChildOf(item), Transform::IDENTITY,
                Aabb::from_min_max(Vec3::new(-0.6, 0.3, 0.1), Vec3::new(-0.4, 0.5, 0.3))));
            app.update();
            app.world_mut().trigger(Interact { entity: item });
            app.update();
            let held = app.world().get::<Held>(item).unwrap();
            assert!(held.center.distance(from.rotation * (from.scale * expected)) < 1e-5);
        }
    }

    // CODEX: validate actual carry transforms under a rotated/scaled parent, at several view angles.
    #[test]
    fn aimed_carry_tracks_half_pitch_and_keeps_collider_center_below_view() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin)).add_systems(Update, carry);
        let camera = Camera { computed: ComputedCameraValues {
            clip_from_view: PerspectiveProjection::default().get_clip_from_view(), ..default()
        }, ..default() };
        let camera_entity = app.world_mut().spawn((camera, Player::default(), Transform::IDENTITY)).id();
        let parent = app.world_mut().spawn(Transform::from_xyz(4.0, 0.2, -1.0)
            .with_rotation(Quat::from_rotation_y(0.6)).with_scale(Vec3::splat(1.5))).id();
        let original = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let local_center = Vec3::new(0.13, -0.02, 0.04);
        let item = app.world_mut().spawn((ChildOf(parent), Transform::IDENTITY,
            Held { rotation: original, scale: Vec3::ONE, center: original * local_center, radius: 0.1, ..default() },
            Holdable { aim_forward: Vec3::NEG_X, ..default() })).id();
        for (yaw, pitch) in [(0.0, 0.0), (1.2, -1.5), (-2.3, 1.5)] {
            let eye = Transform::from_xyz(0.7, 1.68, -0.2)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0));
            *app.world_mut().get_mut::<Transform>(camera_entity).unwrap() = eye;
            app.update();
            app.update();
            let global = app.world().get::<GlobalTransform>(item).unwrap();
            let beam = global.rotation() * Vec3::NEG_X;
            let expected = Quat::from_euler(EulerRot::YXZ, yaw, pitch * 0.5, 0.0) * Vec3::NEG_Z;
            assert!(beam.distance(expected) < 1e-5, "beam must follow yaw and half-pitch");
            let center = global.transform_point(local_center);
            let local = eye.rotation.inverse() * (center - eye.translation);
            let camera = app.world().get::<Camera>(camera_entity).unwrap();
            let ndc = camera.clip_from_view().project_point3(local);
            assert!(ndc.x.abs() < 1e-5 && (ndc.y + 0.1).abs() < 1e-5, "center must be 5% below view: {ndc:?}");
            let distance = local.length();
            app.world_mut().get_mut::<Holdable>(item).unwrap().depth_scale = 1.0;
            app.update();
            let full_center = app.world().get::<GlobalTransform>(item).unwrap().transform_point(local_center);
            assert!((full_center.distance(eye.translation) / distance - 2.0).abs() < 1e-4, "default depth must be halved");
            app.world_mut().get_mut::<Holdable>(item).unwrap().depth_scale = 0.5;
        }
    }

    fn run(app: &mut App, seconds: f32) {
        for _ in 0..(seconds * 60.0) as usize {
            app.update();
        }
    }

    #[test]
    fn dropped_box_rests_on_the_floor() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::diagnostic::DiagnosticsPlugin, TransformPlugin, AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)));
        app.finish();
        app.cleanup();
        app.world_mut().spawn((Transform::from_xyz(0.0, 0.012, 0.0), RigidBody::Static, ColliderConstructor::HalfSpace { outward_normal: Vec3::Y }));
        let cube = app
            .world_mut()
            .spawn((Transform::from_xyz(0.0, 1.0, 0.0), RigidBody::Dynamic))
            .with_child((Transform::from_scale(Vec3::splat(0.062)), Collider::cuboid(2.0, 2.0, 2.0)))
            .id();
        run(&mut app, 2.0);
        let y = app.world().get::<Transform>(cube).unwrap().translation.y;
        assert!((y - 0.074).abs() < 0.02, "rests on the plane, y {y}");
    }
}
