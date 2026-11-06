//! CODEX (GPT-6): black native startup background; see .agents/doc/startup-window.md.
//!
//! X11 normally leaves winit's unpainted window contents undefined. A tiling WM can
//! enlarge the window before Bevy presents at the new size, exposing old desktop
//! pixels. Hide only until the server-side background is installed, not until the
//! scene loads: i3 must map the window to determine its actual tiled dimensions.

use bevy::{
    ecs::system::NonSendMarker,
    prelude::*,
    window::{PrimaryWindow, RawHandleWrapper},
};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

// CODEX: GPU fallback for frames before the scene camera exists. Cameras with a
// custom clear color (including lobby.corn) retain their authored background.
const STARTUP_CLEAR_COLOR: Color = Color::BLACK;
// CODEX: X11 pixel value for black in the lobby's opaque TrueColor window. Keep
// this black even if experimenting with the GPU clear color above.
const X11_BLACK_PIXEL: u64 = 0;

// CODEX: install after DefaultPlugins, before app.run(), so the primary window
// entity exists but winit has not yet created/mapped its native window.
pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(ClearColor(STARTUP_CLEAR_COLOR));
    let world = app.world_mut();
    let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    for mut window in windows.iter_mut(world) {
        window.visible = false;
    }
    app.add_systems(First, prepare_and_show);
}

// CODEX: RawHandleWrapper arrives when winit finishes creation. NonSendMarker
// keeps all Xlib access on the event-loop thread; Added makes this a one-shot per
// native window, so later visibility changes and GPU frames are left alone.
fn prepare_and_show(
    _main_thread: NonSendMarker,
    mut windows: Query<
        (&mut Window, &RawHandleWrapper),
        (With<PrimaryWindow>, Added<RawHandleWrapper>),
    >,
) {
    for (mut window, handles) in &mut windows {
        if let Err(error) = install_x11_background(handles) {
            // CODEX: failure must not strand the app behind a hidden window.
            warn!("Cannot install black X11 startup background: {error}");
        }
        // Bevy's normal window synchronization maps it after background setup.
        window.visible = true;
    }
}

// CODEX: isolate platform FFI here. Wayland needs no Xlib calls; its compositor
// waits for a committed buffer, which Bevy clears using STARTUP_CLEAR_COLOR.
fn install_x11_background(handles: &RawHandleWrapper) -> Result<(), Box<dyn std::error::Error>> {
    let (RawDisplayHandle::Xlib(display), RawWindowHandle::Xlib(window)) =
        (handles.get_display_handle(), handles.get_window_handle())
    else {
        return Ok(());
    };
    let display = display.display.ok_or("Xlib display handle is missing")?;
    let xlib = x11_dl::xlib::Xlib::open()?;

    // SAFETY: the query keeps the native window/display alive, and the caller's
    // NonSendMarker pins us to winit's main thread. Use its existing connection
    // and flush before Bevy maps the window. Do not discard pending X11 events.
    unsafe {
        (xlib.XSetWindowBackground)(display.as_ptr().cast(), window.window, X11_BLACK_PIXEL as _);
        (xlib.XSync)(display.as_ptr().cast(), 0);
    }
    Ok(())
}
