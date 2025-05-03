use bevy::{input::mouse::MouseMotion, prelude::*};

use crate::{scenes::CharacterScene, util::scene_set::{AppSceneSet, SceneSet}};

#[derive(Component, Debug, Default, Clone, Reflect)]
pub struct FlyCam;
#[derive(Default, Debug, Clone, Reflect, Component)]
#[component(storage = "SparseSet")]
pub struct Focused;

#[derive(Debug, Clone, Reflect, Resource)]
pub struct FlyCamConfig{
    pub movement_speed: f32,
    pub sensitivity: f32
}
impl Default for FlyCamConfig{
    fn default() -> Self {
        Self {movement_speed: 10.0, sensitivity: 0.09}
    }
}

#[derive(Debug, Clone, Reflect, Resource)]
pub struct FlyCamKeybinds{
    move_forward: KeyCode,
    move_backward: KeyCode,
    move_left: KeyCode,
    move_right: KeyCode,
    move_ascend: KeyCode,
    move_descend: KeyCode,
    cursor_grab: KeyCode
}
impl Default for FlyCamKeybinds{
    fn default() -> Self {
        Self { 
            move_forward: KeyCode::KeyW, 
            move_backward: KeyCode::KeyS, 
            move_left: KeyCode::KeyA, 
            move_right: KeyCode::KeyD, 
            move_ascend: KeyCode::Space, 
            move_descend: KeyCode::ShiftLeft,
            cursor_grab: KeyCode::Backquote
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct FlyCamPlugin;
impl Plugin for FlyCamPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<FlyCam>()
            .register_type::<Focused>()
            .register_type::<FlyCamConfig>()
            .register_type::<FlyCamKeybinds>()

            .init_resource::<FlyCamConfig>()
            .init_resource::<FlyCamKeybinds>()
            .configure_scene_set(Update, CharacterScene, SceneSet(CharacterScene))
            
            .add_systems(Update, (
                toggle_focused.in_set(SceneSet(CharacterScene)),
                read_flycam_button_inputs.in_set(SceneSet(CharacterScene))
            ));
    }
}

fn toggle_focused(
    mut commands: Commands,
    query: Query<(Entity, Option<&Focused>)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    keybinds: Res<FlyCamKeybinds>
){
    if keyboard_input.just_released(keybinds.cursor_grab) {
        for (entity, focus) in query.iter(){
            if focus.is_some() {commands.entity(entity).remove::<Focused>();}
            else {commands.entity(entity).insert(Focused);}
        }
    }
}

/// Reads in input data, sending an event if there are inputs to process
fn read_flycam_button_inputs(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut mouse_input: EventReader<MouseMotion>,
    keybinds: Res<FlyCamKeybinds>,
    config: Res<FlyCamConfig>,
    time: Res<Time>,
    mut cameras: Query<&mut Transform, (With<Focused>, With<FlyCam>)>
){
    
    let mut movement: Vec3 = Vec3::ZERO;
    if keyboard_input.pressed(keybinds.move_backward) {movement.z += 1.0;}
    if keyboard_input.pressed(keybinds.move_forward) {movement.z -= 1.0;}
    if keyboard_input.pressed(keybinds.move_right) {movement.x += 1.0;}
    if keyboard_input.pressed(keybinds.move_left) {movement.x -= 1.0;}
    if keyboard_input.pressed(keybinds.move_ascend) {movement.y += 1.0;}
    if keyboard_input.pressed(keybinds.move_descend) {movement.y -= 1.0;}
    let mouse: Vec2 = mouse_input.read().map(|e| e.delta).sum();
    movement = if movement == Vec3::ZERO {Vec3::ZERO} else {movement.normalize()};
    if mouse == Vec2::ZERO && movement == Vec3::ZERO {return;}
    // Move cameras
    for mut cam in cameras.iter_mut(){
        let mut trans = movement;
        if cam.translation.y < 2.0 {
            trans.x /= 2.0;
            trans.z /= 2.0;
        }
        if trans.y != 0.0 {
            trans.y *= cam.translation.y.abs() / 10.0;
            trans.y = trans.y.abs().max(0.1) * trans.y.signum();
        }
        let (mut yaw, mut pitch, _) = cam.rotation.to_euler(EulerRot::YXZ);
        pitch -= (config.sensitivity*mouse.y).to_radians();
        yaw -= (config.sensitivity*mouse.x).to_radians();
        pitch = pitch.clamp(-1.54, 1.54);
        let yaw_rot = Quat::from_rotation_y(yaw);
        cam.rotation = yaw_rot*Quat::from_rotation_x(pitch);
        cam.translation += (yaw_rot*trans)*config.movement_speed*time.delta_secs();
    }
}

