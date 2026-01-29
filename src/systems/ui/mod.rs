use avian3d::prelude::PhysicsTime;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};
//pub mod console;
pub mod editor;
pub mod shell;

pub fn toggle_cursor_grab_system(
    mut input: ResMut<ButtonInput<KeyCode>>,
    mut local: Local<bool>,
    mut time: ResMut<Time<avian3d::prelude::Physics>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if input.just_pressed(KeyCode::Backquote) {
        // unpause physics on first mouse grab
        if !*local {
            *local = true;
            time.unpause();
        }

        window.cursor_options.grab_mode = match window.cursor_options.grab_mode {
            CursorGrabMode::None => CursorGrabMode::Locked,
            CursorGrabMode::Confined => CursorGrabMode::Locked,
            CursorGrabMode::Locked => CursorGrabMode::None,
        };

        debug!(?window.cursor_options.grab_mode);
    }
}
