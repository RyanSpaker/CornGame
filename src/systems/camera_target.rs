use std::time::Duration;

pub use bevy::prelude::*;

use crate::ecs::cameras::MainCamera;

#[derive(Debug, Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
#[reflect(Component)]
pub struct CameraFocus;
impl CameraFocus {
    fn on_add(query: Query<&Children, Added<Self>>, mut target: Query<&mut CameraPositionTarget>) {
        for cs in query {
            for c in cs {
                if let Ok(mut target) = target.get_mut(*c) {
                    target.active = true;
                }
            }
        }
    }

    fn on_remove(
        mut removed: RemovedComponents<CameraFocus>,
        mut target: Query<&mut CameraPositionTarget>,
        query: Query<&Children>,
    ) {
        for r in removed.read() {
            if let Ok(cs) = query.get(r) {
                for c in cs {
                    if let Ok(mut target) = target.get_mut(*c) {
                        target.active = false;
                    }
                }
            }
        }
    }
}

/// if this is attached to something then maincamera will smoothly move to match it's global transform
/// active = true will set all others active to false
#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct CameraPositionTarget {
    pub active: bool,
}

impl CameraPositionTarget {
    fn on_change(
        mut query: Query<(Entity, &mut CameraPositionTarget, &GlobalTransform)>,
        mut camera: Query<(Entity, &Transform, Option<&mut Targeting>), With<MainCamera>>,
        mut commands: Commands,
    ) -> Result {
        // NOTE assume MainCamera has no parent
        let (camera_entity, camera_t, targeting) = camera.single_mut()?;

        // make sure only one CameraPositionTarget is active
        // TODO should active be a seperate component
        if let Some((entity, _, _)) = query.iter_mut().find(|v| v.1.active && v.1.is_changed()) {
            for (_, mut other, _) in query.iter_mut().filter(|c| c.0 != entity && c.1.active) {
                other.active = false
            }
        }

        let active = query.iter_mut().find(|v| v.1.active);
        match active {
            Some(active) => {
                let (target, val, gt) = active;
                if !val.is_changed() || targeting.as_ref().is_some_and(|t| t.target == target){
                    // we already started the transition
                    return Ok(());
                }

                let target_position = gt.compute_transform(); // TODO don't recompute every frame.

                let mut new_targeting = Targeting {
                    target,
                    original_position: *camera_t,
                    start_position: *camera_t,
                    target_position,
                    returning: false,
                    timer: Timer::from_seconds(0.4, TimerMode::Once),
                };

                // dbg!(&new_targeting);

                if let Some(targeting) = targeting {
                    new_targeting.original_position = targeting.original_position;

                    // TODO, shorten timer if transition already in effect
                }

                commands.entity(camera_entity).insert(new_targeting);
            }
            None => {
                // return to original position
                if let Some(mut targeting) = targeting {
                    if ! targeting.returning {
                        // dbg!(&targeting.target_position.translation);
                        // dbg!(&camera_t.translation);
                        targeting.returning = true;
                        targeting.timer = Timer::from_seconds(0.4, TimerMode::Once);
                        targeting.start_position = *camera_t;
                        targeting.target_position = targeting.original_position;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Targeting {
    target: Entity,
    original_position: Transform,
    start_position: Transform,
    target_position: Transform,
    returning: bool,

    timer: Timer,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            CameraFocus::on_add,
            CameraFocus::on_remove,
            CameraPositionTarget::on_change,
            update_camera_pos,
        )
            .chain(),
    );
    app.register_type::<CameraFocus>();
    app.register_type::<CameraPositionTarget>();
    app.register_type::<Targeting>();
}

fn update_camera_pos(
    query: Query<(Entity, &mut Targeting, &mut Transform)>,
    gt: Query<&GlobalTransform>,
    time: Res<Time>,
    mut commands: Commands
) {
    for (entity, mut targeting, mut transform) in query {
        targeting.timer.tick(time.delta());

        if !targeting.returning {
            if let Ok(gt) = gt.get(targeting.target) {
                // TODO log error, bc it means race condition in case where target is despawned
                targeting.target_position = gt.compute_transform();

                // XXX moving target will work, but start position won't be updated.
                // -> Instead I should require there always be a camera target
            }
        }

        if targeting.timer.finished() {
            transform.translation = targeting.target_position.translation;
            transform.rotation = targeting.target_position.rotation;

            if targeting.returning {
                // transition to original position finished
                commands.entity(entity).remove::<Targeting>();
            }
            return;
        }

        let f = targeting.timer.fraction();
        let curve = EasingCurve::new(
            targeting.start_position.translation,
            targeting.target_position.translation,
            EaseFunction::SmoothStep,
        );

        // dbg!(transform.translation);
        transform.translation = curve.sample_clamped(f);
        // dbg!(transform.translation);

        // NOTE: I think instead this should work using a look_at target, which is lerped from a position currently being looked at to the target you want centered
        let rot = targeting.start_position.rotation.slerp(
            targeting.target_position.rotation,
            EaseFunction::SmoothStep.sample_clamped(f),
        ).normalize();
        transform.rotation = rot;
    }
}

/// relation ship between camera and it's current controller
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship(relationship_target = Controls)]
pub struct ControlledBy(pub Entity);

/// relation ship between camera and it's current controller
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship_target(relationship = ControlledBy)]
pub struct Controls(Vec<Entity>);