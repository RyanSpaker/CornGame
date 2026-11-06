//! CLAUDE (claude-opus-5-5): 2026-11-02 smooth motion from fixed-step code. Fixed-step code writes a
//! [`Target<T>`]; with [`Interpolate`], `T` moves linearly from where it was when the step ran to the
//! target, over the frames until the next step (so what's drawn is a step behind the fixed code).
//! Without it, `T` takes the target each frame.
//! - inserting a `Target<T>` sets `T` to it: the first assignment
//! - anything else changing `T` is a teleport: the target becomes the new `T`, nothing in between is drawn
//!
//! Scripts: `spawn({ Transform: t, "Target<Transform>": t, Interpolate: {} })` (or `insert`), then write
//! `component(e, "Target<Transform>")[0]` after `yield FixedUpdate`.

use bevy::{
    ecs::component::Mutable,
    prelude::*,
    reflect::{GetTypeRegistration, Typed},
};

pub fn plugin(app: &mut App) {
    app.register_type::<Interpolate>();
    interpolates::<Transform>(app);
}

/// `T` eases to its [`Target<T>`] between fixed steps, instead of jumping to it
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct Interpolate {
    /// false: `T` takes the target as soon as a step sets it, as without `Interpolate`
    pub enabled: bool,
}

impl Default for Interpolate {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// where fixed-step code wants `T`
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub struct Target<T: Interpolable>(pub T);

/// what can be a [`Target`]
pub trait Interpolable: Component<Mutability = Mutable> + Clone + PartialEq + FromReflect + TypePath + Typed + GetTypeRegistration {
    /// `self` moved `s` (0..1) of the way to `to`
    fn lerp(&self, to: &Self, s: f32) -> Self;
}

impl Interpolable for Transform {
    fn lerp(&self, to: &Self, s: f32) -> Self {
        Transform {
            translation: self.translation.lerp(to.translation, s),
            rotation: self.rotation.slerp(to.rotation, s),
            scale: self.scale.lerp(to.scale, s),
        }
    }
}

/// where the interval to the next step starts, the target as this frame's steps found it, and the value
/// last given `T` (anything else in `T` was put there by something else: a teleport)
#[derive(Component)]
struct Eased<T> {
    from: T,
    before: T,
    drawn: T,
}

impl<T: Clone> Eased<T> {
    fn at(value: &T) -> Self {
        Self { from: value.clone(), before: value.clone(), drawn: value.clone() }
    }
}

// CLAUDE
/// `Target<T>` for this `T`: registered for scripts and corn files, and moved each frame
pub fn interpolates<T: Interpolable>(app: &mut App) {
    app.register_type::<Target<T>>().add_observer(assign::<T>).add_systems(
        RunFixedMainLoop,
        (
            before_steps::<T>.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            follow::<T>.in_set(RunFixedMainLoopSystems::AfterFixedMainLoop),
        ),
    );
}

/// the first assignment sets `T`
fn assign<T: Interpolable>(insert: On<Insert<Target<T>>>, mut values: Query<(Option<&mut T>, &Target<T>)>, mut commands: Commands) {
    let Ok((value, target)) = values.get_mut(insert.entity) else { return };
    let to = target.0.clone();
    match value {
        Some(mut value) => *value = to.clone(),
        None => {
            commands.entity(insert.entity).insert(to.clone());
        }
    }
    commands.entity(insert.entity).insert(Eased::at(&to));
}

fn before_steps<T: Interpolable>(mut targets: Query<(&Target<T>, &mut Eased<T>)>) {
    for (target, mut eased) in &mut targets {
        eased.before = target.0.clone();
    }
}

/// after this frame's fixed steps: each `T` is the part of the way from the interval's start to its
/// target that the fixed clock is through the interval. `last_step`: the fixed clock the previous frame
fn follow<T: Interpolable>(
    fixed: Res<Time<Fixed>>,
    mut last_step: Local<std::time::Duration>,
    mut values: Query<(&mut T, &mut Target<T>, &mut Eased<T>, Option<&Interpolate>)>,
) {
    let stepped = fixed.elapsed() != *last_step;
    *last_step = fixed.elapsed();
    let s = fixed.overstep_fraction().clamp(0.0, 1.0);
    for (mut value, mut target, mut eased, interpolate) in &mut values {
        // not what this left there: a teleport (compared, not change ticks: the first assignment, and
        // a write of the same value, aren't teleports)
        if *value != eased.drawn {
            target.0 = value.clone();
            *eased = Eased::at(&value);
            continue;
        }
        if stepped {
            // where the last interval ended (several steps this frame: where the first began)
            eased.from = eased.before.clone();
        }
        let next = match interpolate {
            Some(Interpolate { enabled: true }) => eased.from.lerp(&target.0, s),
            _ => target.0.clone(),
        };
        eased.drawn = next.clone();
        *value = next;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    // CLAUDE
    /// the first assignment and teleports set the value, the frames between steps ease to the target
    #[test]
    fn eases_between_steps() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Time::<Fixed>::from_seconds(0.1))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(Duration::from_millis(25)))
            .add_plugins(plugin)
            // each step moves the target 1 along x
            .add_systems(FixedUpdate, |mut targets: Query<&mut Target<Transform>>| {
                for mut t in &mut targets {
                    t.0.translation.x += 1.0;
                }
            });
        let at = |x| Transform::from_xyz(x, 0.0, 0.0);
        let e = app.world_mut().spawn((at(5.0), Target(at(10.0)), Interpolate::default())).id();
        let x = |app: &App| app.world().get::<Transform>(e).unwrap().translation.x;
        assert_eq!(x(&app), 10.0, "the first assignment");

        // the first frame's delta is 0; then frames of 25ms, a step every 4th
        app.update();
        let mut seen = vec![];
        for _ in 0..12 {
            app.update();
            seen.push(x(&app));
        }
        assert_eq!(app.world().get::<Target<Transform>>(e).unwrap().0.translation.x, 13.0);
        // the step at the 4th frame sets the target to 11: from 10 by quarters, without pausing at steps
        let want = [10.0, 10.0, 10.0, 10.0, 10.25, 10.5, 10.75, 11.0, 11.25, 11.5, 11.75, 12.0];
        assert!(seen.iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-4), "{seen:?}");

        // a teleport: the value stays where it was put, and the target follows it
        app.world_mut().get_mut::<Transform>(e).unwrap().translation.x = -3.0;
        app.update();
        assert_eq!(x(&app), -3.0);
        assert_eq!(app.world().get::<Target<Transform>>(e).unwrap().0.translation.x, -3.0);

        // disabled: the value jumps to each target as the step sets it
        app.world_mut().get_mut::<Interpolate>(e).unwrap().enabled = false;
        let mut seen = vec![];
        for _ in 0..8 {
            app.update();
            seen.push(x(&app));
        }
        assert_eq!(seen, [-3.0, -3.0, -2.0, -2.0, -2.0, -2.0, -1.0, -1.0]);
    }

    // CLAUDE
    /// the first assignment isn't a teleport: a target written before the next frame stands
    #[test]
    fn first_assignment_keeps_later_targets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, plugin));
        let e = app.world_mut().spawn((Transform::from_xyz(5.0, 0.0, 0.0), Target(Transform::from_xyz(10.0, 0.0, 0.0)), Interpolate::default())).id();
        app.world_mut().get_mut::<Target<Transform>>(e).unwrap().0.translation.x = 20.0;
        app.update();
        assert_eq!(app.world().get::<Target<Transform>>(e).unwrap().0.translation.x, 20.0);
        assert_eq!(app.world().get::<Transform>(e).unwrap().translation.x, 10.0, "from the first assignment");
    }
}
