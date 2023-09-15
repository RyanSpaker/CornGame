use bevy::{prelude::*, input::mouse::MouseMotion, window::{PrimaryWindow, CursorGrabMode}};
use super::FlyCam;

#[derive(Resource)]
pub struct CamLookConfig{
    pub sensitivity: f32
}
impl Default for CamLookConfig{
    fn default() -> Self {
        Self { sensitivity: 0.09}
    }
}

#[derive(Resource)]
pub struct CamLookKeyBinds{
    toggle_grab_cursor: KeyCode
}
impl Default for CamLookKeyBinds{
    fn default() -> Self {
        Self { 
            toggle_grab_cursor: KeyCode::Grave 
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum CamLookState{
    #[default]
    Disabled,
    Enabled
}

pub struct CamLookPlugin<T> where T: States + Copy{
    active_state: T
}
impl<T> CamLookPlugin<T> where T: States + Copy{
    pub fn new(active_state: T) -> Self {
        Self {active_state}
    }
}
impl<T> Plugin for CamLookPlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .add_state::<CamLookState>()
            .init_resource::<CamLookConfig>()
            .init_resource::<CamLookKeyBinds>()
            .add_systems(Update, (
                cam_look.run_if(in_state(CamLookState::Enabled))
            ).run_if(in_state(self.active_state)))
            .add_systems(Update, toggle_capture_mouse);
    }
}

fn toggle_capture_mouse(
    input: Res<Input<KeyCode>>,
    keybinds: Res<CamLookKeyBinds>,
    mut window: Query<&mut Window, (With<PrimaryWindow>, Without<FlyCam>)>,
    cur_state: Res<State<CamLookState>>,
    mut next_state: ResMut<NextState<CamLookState>>
){
    if let Ok(mut window) = window.get_single_mut(){
        if input.just_released(keybinds.toggle_grab_cursor){
            match cur_state.get(){
                CamLookState::Enabled => {
                    next_state.set(CamLookState::Disabled);
                    window.cursor.grab_mode = CursorGrabMode::None;
                    window.cursor.visible = true;
                },
                CamLookState::Disabled => {
                    next_state.set(CamLookState::Enabled);
                    window.cursor.grab_mode = CursorGrabMode::Confined;
                    window.cursor.visible = false;
                }
            }
        }
    }
}

fn cam_look(
    config: Res<CamLookConfig>,
    mut mouse: EventReader<MouseMotion>,
    mut camera: Query<&mut Transform, With<FlyCam>>
){
    camera.iter_mut().for_each(|mut transform| {
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        for movement in mouse.iter(){
            pitch -= (config.sensitivity*movement.delta.y).to_radians();
            yaw -= (config.sensitivity*movement.delta.x).to_radians();
        }
        pitch = pitch.clamp(-1.54, 1.54);
        transform.rotation = Quat::from_rotation_y(yaw)*Quat::from_rotation_x(pitch);
    });
}