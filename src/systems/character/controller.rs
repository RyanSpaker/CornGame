use avian3d::math::PI;
use avian3d::prelude::{collider, Collider, ColliderParent, ColliderTransform, LinearVelocity};
use bevy::math::VectorSpace;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy_tnua::prelude::*;

use bevy_tnua::builtins::{TnuaBuiltinCrouch, TnuaBuiltinDash};

use leafwing_input_manager::action_state::ActionState;
use serde::{Deserialize, Serialize};
use crate::systems::physics::ColliderFor;
use crate::ecs::cameras::MainCamera;

use super::animation::MyAnimationState;
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

pub fn look_handler(
    mut query: Query<
        &ActionState<Action>,
        ( 
            Without<crate::ecs::cameras::MainCamera>, 
            With<CornGameCharController> 
        )
    >,
    mut camera: Query<&mut Transform, With<crate::ecs::cameras::MainCamera>>,
    window: Query<&mut Window, With<PrimaryWindow>>,
){
        let Ok(mut camera) = camera.get_single_mut() else {
            return;
        };

        let Ok(mut input) = query.get_single_mut() else {
            return;
        };

        let mut mouse = input.axis_pair(&Action::Pan);
        if let Ok(window) = window.get_single(){
            if window.cursor_options.grab_mode != CursorGrabMode::Locked {
                mouse = default();
            }
        }

        let sensitivity = 0.002;
        let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
        yaw -= sensitivity * mouse.x;
        pitch -= sensitivity * mouse.y;
        pitch = pitch.clamp(-1.54, 1.54);

        let yaw_rot = Quat::from_rotation_y(yaw);
        camera.rotation = yaw_rot * Quat::from_rotation_x(pitch);

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
            &mut MyAnimationState,
        ),
        Without<crate::ecs::cameras::MainCamera>,
    >,
    mut colliders: Query<(&ColliderParent, &mut Collider, &mut Transform), (Without<CornGameCharController>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, With<crate::ecs::cameras::MainCamera>>,
    mut window: Query<&mut Window, With<PrimaryWindow>>,
) {
    let Ok(mut camera) = camera.get_single_mut() else {
        return;
    };

    assert!( query.iter().count() <= 1);
    for (id, transform, input, mut controller, config, mut anim_state) in query.iter_mut() {
        let collider = colliders.iter_mut().find(|c|c.0.get() == id);
        
        if collider.is_none() {
            commands.spawn((Collider::default(),Transform::default())).set_parent(id);
            continue;
        };

        let (_, mut collider, mut c_trans) = collider.unwrap();

        let mut direction = input.clamped_axis_pair(&Action::Move);

        if direction == Vec2::ZERO {
            anim_state.set_if_neq(MyAnimationState::Idle);
        }else{
            anim_state.set_if_neq(MyAnimationState::Walk(direction));
        }

        // Only allow input when cursor is grabbed
        // TODO, we should do this on the input side instead of here.
        // TODO need a generic framework for claiming inputs
        if let Ok(window) = window.get_single(){
            if window.cursor_options.grab_mode != CursorGrabMode::Locked {
                direction = default();
            }
        }

        camera.translation = transform.translation + Vec3::new(0., 0.0, 0.);

        let mut direction = direction.extend(0.0).xzy();
        direction.z = -direction.z;
        let (yaw,_,_) = camera.rotation.to_euler(EulerRot::YXZ);
        direction = Quat::from_euler(EulerRot::YXZ, yaw, 0.0, 0.0) * direction;

        let speed = match input.pressed(&Action::Run) {
            true => config.dash_speed,
            false => config.speed,
        };

        // TODO add math helpers
        let forward = camera.forward().reject_from(Vec3::Y).normalize_or_zero();

        let basis = TnuaBuiltinWalk {
            desired_velocity: direction * speed,
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

pub fn round_velocity(
    mut query: Query<&mut LinearVelocity, With<CornGameCharController> >
){
    for mut v in query.iter_mut(){
        if v.0 != Vec3::ZERO && v.length() < 0.0001 {
            // v.0 = Vec3::ZERO;
        }
    }
}
                // |q:Query<(Entity, &LinearVelocity), With<CornGameCharController>>|{ 
                //     for (i,v) in q.iter(){
                //         info!("{} {:?}", i, v);
                //     }
                // }