//! CLAUDE (claude-opus-5-5): interaction ranges. An [`InteractionRange`] is a shape around its entity;
//! a [`RangeTarget`] (the player) coming inside fires [`RangeEnter`], leaving (or despawning) fires
//! [`RangeExit`], and moving while inside fires [`RangeMove`]. Scripts listen with
//! `on RangeEnter: open;`, the event's `other` is the target.
//!
//! Made to swap onto avian sensors later. Avian's own events don't fit as they are: a zone is
//! `Collider + Sensor + CollisionEventsEnabled` and gets `OnCollisionStart/End { collider, body }`, but
//! there's no move event, filtering is only by collision layers, and a target despawning inside seems
//! to send no end to the zone (the ended event is read after the entity is gone). So the split here is:
//! - a backend keeps [`InRange`], who is inside each range: here a point test against the shape; with
//!   avian, `CollidingEntities` on the sensor, mapped to bodies (`body ?? collider`) and filtered to
//!   targets. The shape becomes the sensor's collider.
//! - [`fire`] compares it with last frame and triggers the events. It stays the same.
//!
//! In one frame every exit fires before any enter, so moving from one range straight into another (a
//! portal) ends with the enter's handler. The demo tests the target's origin, avian would test its
//! whole collider, so with avian ranges trigger a little sooner.

use bevy::{ecs::entity::EntityHashSet, prelude::*};
use corn_scripting::ScriptEventsExt;

pub fn plugin(app: &mut App) {
    app.register_type::<InteractionRange>()
        .register_type::<InRange>()
        .register_type::<RangeTarget>()
        .register_script_entity_event::<RangeEnter>()
        .register_script_entity_event::<RangeExit>()
        .register_script_entity_event::<RangeMove>()
        // after transforms settle; script handlers run in the next frame's driver
        .add_systems(PostUpdate, (contain, fire).chain().after(TransformSystems::Propagate));
}

/// the shape around this entity (in its local space, so it turns and scales with it) that
/// [`RangeTarget`]s enter. `InteractionRange: Sphere { radius: 3 };`
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
#[require(InRange, Transform)]
pub enum InteractionRange {
    Sphere(Sphere),
    Cuboid(Cuboid),
    /// along Y
    Cylinder(Cylinder),
    /// along Y
    Capsule(Capsule3d),
}

impl Default for InteractionRange {
    fn default() -> Self {
        Self::Sphere(Sphere::default())
    }
}

impl InteractionRange {
    /// whether `point`, in the range's local space, is inside
    pub fn contains(&self, point: Vec3) -> bool {
        match self {
            Self::Sphere(s) => point.length_squared() <= s.radius * s.radius,
            Self::Cuboid(c) => point.abs().cmple(c.half_size).all(),
            Self::Cylinder(c) => point.xz().length_squared() <= c.radius * c.radius && point.y.abs() <= c.half_height,
            Self::Capsule(c) => {
                let axis = Vec3::new(0.0, point.y.clamp(-c.half_length, c.half_length), 0.0);
                point.distance_squared(axis) <= c.radius * c.radius
            }
        }
    }
}

/// what ranges notice: the player
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct RangeTarget;

/// the targets inside this range, kept by the backend. scripts may read it
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
pub struct InRange(pub Vec<Entity>);

/// `other` came inside the range `entity`
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct RangeEnter {
    pub entity: Entity,
    pub other: Entity,
}

/// `other` left the range `entity`, or despawned inside it
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct RangeExit {
    pub entity: Entity,
    pub other: Entity,
}

/// `other` moved this frame while inside the range `entity`
#[derive(EntityEvent, Reflect, Clone, Debug)]
pub struct RangeMove {
    pub entity: Entity,
    pub other: Entity,
}

/// who was inside each range last frame
#[derive(Component, Default)]
struct WasInRange(EntityHashSet);

/// the backend: tests each target's position against each range's shape
fn contain(mut ranges: Query<(&InteractionRange, &GlobalTransform, &mut InRange)>, targets: Query<(Entity, &GlobalTransform), With<RangeTarget>>) {
    for (range, transform, mut inside) in &mut ranges {
        let to_local = transform.affine().inverse();
        let now: Vec<Entity> = targets
            .iter()
            .filter(|(_, target)| range.contains(to_local.transform_point3(target.translation())))
            .map(|(entity, _)| entity)
            .collect();
        // only touch it when it changes, so `Changed<InRange>` means something
        if inside.0 != now {
            inside.0 = now;
        }
    }
}

/// triggers the events from how [`InRange`] changed: every exit, then every enter, then moves
fn fire(
    mut commands: Commands,
    mut ranges: Query<(Entity, &InRange, Option<&mut WasInRange>)>,
    moved: Query<(), Changed<GlobalTransform>>,
) {
    let (mut exits, mut enters, mut moves) = (vec![], vec![], vec![]);
    for (range, inside, was) in &mut ranges {
        let now: EntityHashSet = inside.0.iter().copied().collect();
        let Some(mut was) = was else {
            enters.extend(now.iter().map(|&other| RangeEnter { entity: range, other }));
            commands.entity(range).insert(WasInRange(now));
            continue;
        };
        exits.extend(was.0.difference(&now).map(|&other| RangeExit { entity: range, other }));
        for &other in &inside.0 {
            if !was.0.contains(&other) {
                enters.push(RangeEnter { entity: range, other });
            } else if moved.contains(other) {
                moves.push(RangeMove { entity: range, other });
            }
        }
        if was.0 != now {
            was.0 = now;
        }
    }
    exits.into_iter().for_each(|e| commands.trigger(e));
    enters.into_iter().for_each(|e| commands.trigger(e));
    moves.into_iter().for_each(|e| commands.trigger(e));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// events seen, as (kind, range, other)
    #[derive(Resource, Default)]
    struct Seen(Vec<(&'static str, Entity, Entity)>);

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, corn_scripting::plugin, plugin));
        app.init_resource::<Seen>();
        app.add_observer(|on: On<RangeEnter>, mut seen: ResMut<Seen>| seen.0.push(("enter", on.entity, on.other)));
        app.add_observer(|on: On<RangeExit>, mut seen: ResMut<Seen>| seen.0.push(("exit", on.entity, on.other)));
        app.add_observer(|on: On<RangeMove>, mut seen: ResMut<Seen>| seen.0.push(("move", on.entity, on.other)));
        app
    }

    fn step(app: &mut App) -> Vec<(&'static str, Entity, Entity)> {
        app.update();
        std::mem::take(&mut app.world_mut().resource_mut::<Seen>().0)
    }

    fn put(app: &mut App, entity: Entity, x: f32) {
        app.world_mut().get_mut::<Transform>(entity).unwrap().translation.x = x;
    }

    #[test]
    fn shapes() {
        let sphere = InteractionRange::Sphere(Sphere::new(1.0));
        assert!(sphere.contains(Vec3::new(0.0, 0.9, 0.0)) && !sphere.contains(Vec3::new(0.8, 0.8, 0.0)));
        let cuboid = InteractionRange::Cuboid(Cuboid::new(2.0, 4.0, 2.0));
        assert!(cuboid.contains(Vec3::new(0.9, 1.9, -0.9)) && !cuboid.contains(Vec3::new(0.0, 2.1, 0.0)));
        let cylinder = InteractionRange::Cylinder(Cylinder::new(1.0, 2.0));
        assert!(cylinder.contains(Vec3::new(0.7, 0.9, 0.7)) && !cylinder.contains(Vec3::new(0.0, 1.1, 0.0)));
        let capsule = InteractionRange::Capsule(Capsule3d::new(1.0, 2.0));
        assert!(capsule.contains(Vec3::new(0.0, 1.9, 0.0)) && !capsule.contains(Vec3::new(0.9, 1.9, 0.0)));
    }

    #[test]
    fn enter_move_exit() {
        let mut app = app();
        let range = app.world_mut().spawn((InteractionRange::Sphere(Sphere::new(1.0)), Transform::default())).id();
        let player = app.world_mut().spawn((RangeTarget, Transform::from_xyz(5.0, 0.0, 0.0))).id();
        // something that isn't a target never counts
        app.world_mut().spawn(Transform::default());
        assert_eq!(step(&mut app), []);

        put(&mut app, player, 0.5);
        assert_eq!(step(&mut app), [("enter", range, player)]);
        assert_eq!(app.world().get::<InRange>(range).unwrap().0, [player]);
        assert_eq!(step(&mut app), [], "standing still");
        put(&mut app, player, 0.2);
        assert_eq!(step(&mut app), [("move", range, player)]);
        put(&mut app, player, 3.0);
        assert_eq!(step(&mut app), [("exit", range, player)]);

        // despawning inside is leaving
        put(&mut app, player, 0.0);
        step(&mut app);
        app.world_mut().despawn(player);
        assert_eq!(step(&mut app), [("exit", range, player)]);
    }

    #[test]
    fn ranges_turn_and_start_full() {
        let mut app = app();
        let player = app.world_mut().spawn((RangeTarget, Transform::from_xyz(0.0, 0.0, 1.5))).id();
        // a long box along X, turned to lie along Z: already holding the player when it appears
        let turned = Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        let range = app.world_mut().spawn((InteractionRange::Cuboid(Cuboid::new(4.0, 1.0, 1.0)), turned)).id();
        assert_eq!(step(&mut app), [("enter", range, player)]);
    }

    #[test]
    fn exits_before_enters() {
        let mut app = app();
        let player = app.world_mut().spawn((RangeTarget, Transform::default())).id();
        let range = |x: f32| (InteractionRange::Sphere(Sphere::new(1.0)), Transform::from_xyz(x, 0.0, 0.0));
        // spawned in this order so the enter's range comes first when ranges are visited
        let inside = app.world_mut().spawn(range(10.0)).id();
        let outside = app.world_mut().spawn(range(0.0)).id();
        step(&mut app);
        // a portal: out of one, into the other, in one frame
        put(&mut app, player, 10.0);
        assert_eq!(step(&mut app), [("exit", outside, player), ("enter", inside, player)]);
    }
}
