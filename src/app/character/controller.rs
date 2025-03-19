use avian3d::math::PI;
use avian3d::prelude::{collider, Collider, ColliderParent, ColliderTransform};
use bevy::prelude::*;
use bevy_tnua::prelude::*;

use bevy_tnua::builtins::{TnuaBuiltinCrouch, TnuaBuiltinDash};

use leafwing_input_manager::action_state::ActionState;
use serde::{Deserialize, Serialize};
use crate::app::physics::ColliderFor;
use crate::ecs::main_camera::MainCamera;

use super::input::Action;

#[derive(Debug, Component, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
#[require(TnuaController)]
pub struct CornGameCharController {
    pub crouch_height: f32,
    pub height: f32,
    pub radius: f32,
    pub float: f32,
    pub crouch_float: f32,

    pub eye_height: f32,
    pub max_slope: f32,

    pub acceleration: f32,
    pub speed: f32,
    pub dash_speed: f32,

    pub spring: f32,
    pub crouch_duration: f32,
}

impl Default for CornGameCharController {
    fn default() -> Self {
        Self {
            crouch_height: 1.0,
            height: 2.0,
            radius: 0.3,
            float: 0.25,
            crouch_float: 0.1,
            eye_height: 1.5,
            max_slope: PI / 3.0,
            acceleration: 150.0,
            speed: 2.5,
            dash_speed: 5.0,

            spring: 400.0,
            crouch_duration: 0.08,
        }
    }
}

// controls the main camera and the Player entity (these are intractibly linked)
// camera should not be a child of player, you need flexibility to decouple these
// the player *can* have a mesh, but it might not (on other clients for example), but lets assume for now character controller only runs for player and server
pub fn input_handler(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Transform,
            &ActionState<Action>,
            &mut TnuaController,
            &CornGameCharController,
        ),
        Without<crate::ecs::main_camera::MainCamera>,
    >,
    mut colliders: Query<(&ColliderParent, &mut Collider, &mut Transform), (Without<CornGameCharController>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, With<crate::ecs::main_camera::MainCamera>>,
) {
    let Ok(mut camera) = camera.get_single_mut() else {
        return;
    };

    for (id, transform, input, mut controller, config) in query.iter_mut() {
        let collider = colliders.iter_mut().find(|c|c.0.get() == id);
        
        if collider.is_none() {
            commands.spawn((Collider::default(),Transform::default())).set_parent(id);
            continue;
        };

        let (_, mut collider, mut c_trans) = collider.unwrap();

        let mouse = input.axis_pair(&Action::Pan);

        let sensitivity = 0.002;
        let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
        yaw -= sensitivity * mouse.x;
        pitch -= sensitivity * mouse.y;
        pitch = pitch.clamp(-1.54, 1.54);

        let yaw_rot = Quat::from_rotation_y(yaw);
        camera.rotation = yaw_rot * Quat::from_rotation_x(pitch);
        camera.translation = transform.translation + Vec3::new(0., 0.0, 0.);

        let direction = input.clamped_axis_pair(&Action::Move);
        let direction: Vec2 = direction.into();

        let mut direction = direction.extend(0.0).xzy();
        direction.z = -direction.z;

        let speed = match input.pressed(&Action::Run) {
            true => config.dash_speed,
            false => config.speed,
        };

        // TODO add math helpers
        let forward = camera.forward().reject_from(Vec3::Y).normalize_or_zero();

        let basis = TnuaBuiltinWalk {
            desired_velocity: yaw_rot * direction * speed,
            desired_forward: forward.try_into().ok(),
            acceleration: config.acceleration,
            float_height: config.eye_height,
            max_slope: config.max_slope,
            spring_strengh: config.spring,
            ..default()
        };
        controller.basis(basis);

        let height = config.height - config.float;
        *collider = Collider::capsule(config.radius, height - 2.0*config.radius);
        let offset = config.eye_height - height / 2.0 - config.float;
        *c_trans = Transform::from_xyz(0.0, -offset, 0.0);

        if input.pressed(&Action::Crouch) {
            let c_height = config.crouch_height - config.crouch_float;
            *collider = Collider::capsule(config.radius, c_height - 2.0*config.radius);
            let c_eye_height = config.eye_height + config.crouch_height - config.height;
            let c_offset = c_eye_height - c_height / 2.0 - config.crouch_float;
            *c_trans = Transform::from_xyz(0.0, -c_offset, 0.0);
            
            controller.action(TnuaBuiltinCrouch {
                float_offset: config.crouch_height - config.height,
                height_change_impulse_for_duration: config.crouch_duration,
                ..Default::default()
            });
        } else {
            // why is this needed?
            // if input.pressed(&Action::Run) {
            //     controller.action(TnuaBuiltinDash::default());
            // }
        }

    }
}
