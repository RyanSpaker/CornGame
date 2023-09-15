use bevy::prelude::*;
use super::cam_look_plugin::CamLookState;
use super::FlyCam;

#[derive(Resource)]
pub struct CamMoveConfig{
    pub movement_speed: f32
}
impl Default for CamMoveConfig{
    fn default() -> Self {
        Self {movement_speed: 10.0}
    }
}

#[derive(Resource)]
pub struct CamMoveKeybinds{
    move_forward: KeyCode,
    move_backward: KeyCode,
    move_left: KeyCode,
    move_right: KeyCode,
    move_ascend: KeyCode,
    move_descend: KeyCode
}
impl Default for CamMoveKeybinds{
    fn default() -> Self {
        Self { 
            move_forward: KeyCode::W, 
            move_backward: KeyCode::S, 
            move_left: KeyCode::A, 
            move_right: KeyCode::D, 
            move_ascend: KeyCode::Space, 
            move_descend: KeyCode::ShiftLeft
        }
    }
}

pub struct CamMovePlugin<T> where T: States + Copy{
    active_state: T
}
impl<T> CamMovePlugin<T> where T: States + Copy{
    pub fn new(active_state: T) -> Self {
        Self {active_state}
    }
}
impl<T> Plugin for CamMovePlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CamMoveConfig>()
            .init_resource::<CamMoveKeybinds>()
            .add_systems(Update, (
                cam_move.run_if(in_state(CamLookState::Enabled))
            ).run_if(in_state(self.active_state)));
    }
}

fn cam_move(
    input: Res<Input<KeyCode>>,
    keybinds: Res<CamMoveKeybinds>,
    config: Res<CamMoveConfig>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<FlyCam>>
){
    let z: i32 = input.pressed(keybinds.move_backward) as i32 - input.pressed(keybinds.move_forward) as i32;
    let x: i32 = input.pressed(keybinds.move_right) as i32 - input.pressed(keybinds.move_left) as i32;
    let y: i32 = input.pressed(keybinds.move_ascend) as i32 - input.pressed(keybinds.move_descend) as i32;
    if x!=0 || y!=0 || z!=0{
        let mut dir: Vec3 = Vec3{x: x as f32, y: y as f32, z: z as f32}.normalize();
        for mut transform in query.iter_mut(){
            dir = Quat::from_rotation_y(transform.rotation.to_euler(EulerRot::YXZ).0)*dir;
            dir *= config.movement_speed*time.delta_seconds();
            transform.translation += dir;
        }
    }
}