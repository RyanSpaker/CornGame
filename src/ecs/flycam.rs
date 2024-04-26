use bevy::{
    app::{App, Plugin, PreUpdate, Update}, 
    ecs::{component::Component, 
        event::{Event, EventReader, EventWriter}, 
        query::{With, Without}, 
        schedule::{common_conditions::{in_state, not, on_event}, AndThen, IntoSystemConfigs, NextState, OrElse, States}, 
        system::{IntoSystem, Query, Res, ResMut, Resource}
    }, 
    input::{keyboard::KeyCode, mouse::MouseMotion, ButtonInput}, 
    math::{EulerRot, Quat, Vec2, Vec3}, 
    prelude::System, 
    reflect::Reflect, 
    time::Time, 
    transform::components::Transform, 
    window::{CursorGrabMode, PrimaryWindow, Window}
};

use crate::util::lerp;

use super::corn::field::cf_image_carved::CornSensor;

#[derive(Component, Debug, Default, Clone, Reflect)]
pub struct FlyCam;

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
            cursor_grab: KeyCode::Escape
        }
    }
}

#[derive(Default, Debug, Clone, Reflect, Hash, PartialEq, Eq, States)]
pub enum FlyCamState{
    Focused,
    Unfocused,
    #[default]
    Disabled
}

#[derive(Default, Debug, Clone, Reflect, Event)]
pub struct FlyCamMoveEvent(pub Vec3);
#[derive(Default, Debug, Clone, Reflect, Event)]
pub struct FlyCamCaptureEvent;

pub struct FlyCamPlugin{
    initial_state: FlyCamState
}
impl FlyCamPlugin{
    pub fn new(initial_state: FlyCamState) -> Self {Self{initial_state}}
}
impl Plugin for FlyCamPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<FlyCam>()
            .register_type::<FlyCamConfig>()
            .register_type::<FlyCamKeybinds>()
            .register_type::<FlyCamState>()
            .init_resource::<FlyCamConfig>()
            .init_resource::<FlyCamKeybinds>()
            .insert_state::<FlyCamState>(self.initial_state.clone())
            .add_event::<FlyCamMoveEvent>()
            .add_event::<FlyCamCaptureEvent>()
            .add_systems(PreUpdate, read_flycam_button_inputs.run_if(not(in_state(FlyCamState::Disabled))))
            .add_systems(Update, (
                capture_mouse.run_if(and(
                    not(in_state(FlyCamState::Disabled)), 
                    on_event::<FlyCamCaptureEvent>())
                ),
                move_cam.run_if(and(
                    in_state(FlyCamState::Focused), 
                    or(on_event::<MouseMotion>(), on_event::<FlyCamMoveEvent>())
                ))
            ));
    }
}

pub fn and<Marker, TOut, T, MarkerB, BOut, B>(condition: T, condition_b: B) -> AndThen<T::System, B::System>
where
    T: IntoSystem<(), TOut, Marker>,
    B: IntoSystem<(), BOut, MarkerB>,
{
    let condition = IntoSystem::into_system(condition);
    let condition_b = IntoSystem::into_system(condition_b);
    let name = format!("{}&&{}", condition.name(), condition_b.name());
    AndThen::new(condition, condition_b, name.into())
}

pub fn or<Marker, TOut, T, MarkerB, BOut, B>(condition: T, condition_b: B) -> OrElse<T::System, B::System>
where
    T: IntoSystem<(), TOut, Marker>,
    B: IntoSystem<(), BOut, MarkerB>,
{
    let condition = IntoSystem::into_system(condition);
    let condition_b = IntoSystem::into_system(condition_b);
    let name = format!("{}||{}", condition.name(), condition_b.name());
    OrElse::new(condition, condition_b, name.into())
}

pub fn enable_flycam(mut next_state: ResMut<NextState<FlyCamState>>){
    next_state.set(FlyCamState::Unfocused);
}
/// Reads in input data, sending an event if there are inputs to process
fn read_flycam_button_inputs(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    keybinds: Res<FlyCamKeybinds>,
    mut move_events: EventWriter<FlyCamMoveEvent>,
    mut capture_events: EventWriter<FlyCamCaptureEvent>
){

    if keyboard_input.just_released(keybinds.cursor_grab) {
        capture_events.send(FlyCamCaptureEvent);
    }
    let mut movement: Vec3 = Vec3::ZERO;
    if keyboard_input.pressed(keybinds.move_backward) {movement.z += 1.0;}
    if keyboard_input.pressed(keybinds.move_forward) {movement.z -= 1.0;}
    if keyboard_input.pressed(keybinds.move_right) {movement.x += 1.0;}
    if keyboard_input.pressed(keybinds.move_left) {movement.x -= 1.0;}
    if keyboard_input.pressed(keybinds.move_ascend) {movement.y += 1.0;}
    if keyboard_input.pressed(keybinds.move_descend) {movement.y -= 1.0;}
    if movement == Vec3::ZERO {return;}
    move_events.send(FlyCamMoveEvent(movement.normalize()));
}
/// Toggles the Capture of the mouse when the grave key is pressed
fn capture_mouse(
    mut window: Query<&mut Window, (With<PrimaryWindow>, Without<FlyCam>)>,
    mut next_state: ResMut<NextState<FlyCamState>>
){
    if let Ok(mut window) = window.get_single_mut(){
        match window.cursor.grab_mode {
            CursorGrabMode::None => {
                window.cursor.grab_mode = CursorGrabMode::Locked;
                window.cursor.visible = false;
                next_state.set(FlyCamState::Focused);
            },
            CursorGrabMode::Confined => {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
                next_state.set(FlyCamState::Unfocused);
            },
            CursorGrabMode::Locked => {
                window.cursor.grab_mode = CursorGrabMode::None;
                window.cursor.visible = true;
                next_state.set(FlyCamState::Unfocused);
            }
        };
    }
}
/// Moves the camera reading from move and mouse motion events
fn move_cam(
    mut move_events: EventReader<FlyCamMoveEvent>,
    mut mouse_events: EventReader<MouseMotion>,
    config: Res<FlyCamConfig>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, Option<&CornSensor>), With<FlyCam>>
){
    let move_events: Vec<&FlyCamMoveEvent> = move_events.read().collect();
    let mouse_events: Vec<&MouseMotion> = mouse_events.read().collect();
    let is_mouse = !mouse_events.is_empty(); let is_move = !move_events.is_empty();
    if !is_move && !is_mouse {return;}
    let total_mouse: Vec2 = mouse_events.into_iter().map(|event| event.delta).sum();
    let mut total_move: Vec3 = move_events.into_iter().map(|event| event.0).sum::<Vec3>().normalize();
    for (mut transform, in_corn) in query.iter_mut(){
        
        let is_in_corn = in_corn.cloned().unwrap_or_default().is_in_corn;

        if transform.translation.y < 2.0 {
            if is_in_corn != 0.0 {
                let factor = lerp(1.0, 0.3, is_in_corn);
                total_move.x *= factor;
                total_move.z *= factor;
            }
            total_move.x /= 2.0;
            total_move.z /= 2.0;
        }

        // less speed near ground
        if total_move.y != 0.0 {
            total_move.y *= transform.translation.y.abs() / 10.0;
            total_move.y = total_move.y.abs().max(0.1) * total_move.y.signum();
        }
        
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        if is_mouse {
            pitch -= (config.sensitivity*total_mouse.y).to_radians();
            yaw -= (config.sensitivity*total_mouse.x).to_radians();
            pitch = pitch.clamp(-1.54, 1.54);
        }
        let yaw_rot = Quat::from_rotation_y(yaw);
        if is_mouse {
            transform.rotation = yaw_rot*Quat::from_rotation_x(pitch);
        }
        if is_move{
            let movement: Vec3 = yaw_rot*total_move;
            transform.translation += movement*config.movement_speed*time.delta_seconds();
        }
    }
}