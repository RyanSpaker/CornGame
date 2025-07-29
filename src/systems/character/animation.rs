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


use std::time::Duration;

pub use bevy::prelude::*;
use frunk::labelled::chars::Q;
use lightyear::prelude::AppComponentExt;
use serde::{Deserialize, Serialize};

use crate::systems::animation_context::AnimationParams;

#[derive(Debug, Clone, Component, Reflect, PartialEq, Serialize, Deserialize)]
#[reflect(Component)]
pub enum MyAnimationState {
    Idle,
    Walk(Vec2),
}

#[expect(unused)]
impl MyAnimationState {
    fn update_animation(
        query: Query<(Entity, &MyAnimationState), Changed<MyAnimationState>>,
        children: Query<&Children>,
        mut animation: Query<(AnimationParams)>,
    ) {
        for (id, state) in query.iter() {
            // a bunch of ugly code to let player model be a child of the controller
            let mut ids = vec![id];
            for id in children.iter_descendants(id){
                let Ok(mut animation) = animation.get_mut(id) else {
                    continue;
                };

                let anim_name = match state {
                    MyAnimationState::Idle => "idle",
                    MyAnimationState::Walk(_vec2) => "walk",
                };

                if let Ok(active) = animation.play(anim_name, Duration::from_millis(200)) {
                    active.repeat();
                }

                break;
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.register_type::<MyAnimationState>();
    app.add_systems(Update, MyAnimationState::update_animation);
}
