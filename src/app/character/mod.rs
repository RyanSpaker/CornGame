/// This will implement the character controller and animations.
/// 
/// using a library called tnua because it had a working demo with animations. https://idanarye.github.io/bevy-tnua/demos/platformer_3d-xpbd
/// might want to rip it out eventually
/// 
/// TODO:
/// [ ] character controller
/// [ ] animations
/// [ ] networking
///   [ ] animation
///   [ ] interpolation
/// [ ] item holding (ex flashlight)
/// [ ] sight map (for out of sight changes)

use bevy::prelude::*;
use bevy_xpbd_3d::prelude::*;
use leafwing_input_manager::plugin::InputManagerPlugin;
use leafwing_input_manager::InputManagerBundle;

use self::input::Action;

mod input;
mod controller;
mod animation;

pub struct MyCharacterPlugin;
impl Plugin for MyCharacterPlugin{
    fn build(&self, app: &mut App) {
        // button input plugin
        app.add_plugins(InputManagerPlugin::<self::input::Action>::default());
        
        // init character controller plugin
        app.add_plugins(bevy_tnua_xpbd3d::TnuaXpbd3dPlugin::default());
        app.add_plugins(bevy_tnua::controller::TnuaControllerPlugin::default());

        // This plugin supports `TnuaCrouchEnforcer`, which prevents the character from standing up
        // while obstructed by an obstacle.
        // app.add_plugins(bevy_tnua::control_helpers::TnuaCrouchEnforcerPlugin::default());

        // app.add_systems(Startup, setup_player);
        app.add_systems(
            Update,
            self::controller::input_handler.in_set(bevy_tnua::TnuaUserControlsSystemSet),
        );
        // app.add_systems(Update, animation_patcher_system);
        // app.add_systems(Update, animate_platformer_character);
    }
}

/// Anything that moves via tnua character controller -- might be NPC, might be other players
pub struct Player;
impl Player {
    pub fn bundle(&self) -> impl Bundle {
        (
            RigidBody::Dynamic,
            Collider::capsule(1.5, 0.5),
            bevy_tnua::controller::TnuaControllerBundle::default(),
            bevy_tnua::TnuaAnimatingState::<self::animation::AnimationState>::default(),
                
            InputManagerBundle::with_map(
                Action::default_input_map(),
            ),

            Name::new("Player")

            // NOTE: not bothering with this yet
            // `TnuaCrouchEnforcer` can be used to prevent the character from standing up when obstructed.
            // bevy_tnua::control_helpers::TnuaCrouchEnforcer::new(0.5 * Vec3::Y, |cmd| {
            //     cmd.insert(bevy_tnua_xpbd3d::TnuaXpbd3dSensorShape(
            //         Collider::cylinder(0.0, 0.5)));
            // })
        )
    }
}

// TODO system to prevent rendering player model for current player
// TODO system to rehydrate player on client