
// reference https://github.com/bevyengine/bevy/blob/main/examples/animation/animation_graph.rs
// https://github.com/idanarye/bevy-tnua/blob/main/demos/src/character_animating_systems/platformer_animating_systems.rs

// TODO
// [ ] Inverse kinematics
//   [ ] camera direction and held item
// [ ] animation graph assets
// [ ] animation transitions and tweening
// [ ] networking animation state/inputs
// [ ] sharing code between NPC's and player character controller

// simplest way to do animation?
// just base it on the

pub use bevy::prelude::*;

#[derive(Debug)]
pub enum AnimationState {
    Standing,
    Running(f32),
    Jumping,
    Falling,
    Crouching,
    Crawling(f32),
    Dashing,
}

pub struct MyAnimationPlugin;

impl Plugin for MyAnimationPlugin {
    fn build(&self, app: &mut App) {
        //app.add_plugins(AnimationGraphPlugin);
    }
}

