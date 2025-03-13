
use bevy::prelude::*;
use bevy_tnua::prelude::*;

use bevy_tnua::builtins::{TnuaBuiltinCrouch, TnuaBuiltinDash};

use leafwing_input_manager::action_state::ActionState;
use super::input::Action; 

// controls the main camera and the Player entity (these are intractibly linked)
// camera should not be a child of player, you need flexibility to decouple these
// the player *can* have a mesh, but it might not (on other clients for example), but lets assume for now character controller only runs for player and server
pub fn input_handler(
    mut query : Query<(&Transform, &ActionState<Action>, &mut TnuaController), Without<crate::ecs::main_camera::MainCamera>>,
    mut camera : Query<&mut Transform, With<crate::ecs::main_camera::MainCamera>>,
){
    let Ok(mut camera) = camera.get_single_mut() else { return };

    for (transform, input, mut controller) in query.iter_mut() {
        let mouse = input.axis_pair(&Action::Pan);

        let sensitivity = 0.002;
        let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
        yaw   -= sensitivity * mouse.x;
        pitch -= sensitivity * mouse.y;
        pitch = pitch.clamp(-1.54, 1.54);

        let yaw_rot = Quat::from_rotation_y(yaw);
        camera.rotation = yaw_rot*Quat::from_rotation_x(pitch);
        camera.translation = transform.translation + Vec3::new(0., 0.0, 0.);

        let direction = input.clamped_axis_pair(&Action::Move);
        let direction : Vec2 = direction.into();

        let mut direction = direction.extend(0.0).xzy();
        direction.z = -direction.z;

        let speed = match input.pressed(&Action::Run) {
            true => 5.0,
            false => 2.5,
        };

        // TODO add math helpers
        let forward = camera.forward().reject_from(Vec3::Y).normalize_or_zero();

        controller.basis( TnuaBuiltinWalk{
            desired_velocity: yaw_rot * direction * speed,
            desired_forward: forward.try_into().ok(),
            float_height: 2.0,
            acceleration: 150.0,
            ..default()
        });

        if input.pressed(&Action::Crouch){
            controller.action(TnuaBuiltinCrouch {
                float_offset: -0.9,
                ..Default::default()
            });
        } else if input.pressed(&Action::Run){
            controller.action(TnuaBuiltinDash::default());
        }
    }
}

