//! CODEX (GPT-6): browser start/pause menu handshake; native input keeps its existing behavior.
use bevy::prelude::*;

// CODEX: a cursor flag can outlive browser Escape, so web input trusts the DOM's actual lock.
pub fn captured() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.pointer_lock_element())
            .is_some_and(|element| element.id() == "lobby-canvas")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

// CODEX: DOM owns the overlay; Bevy decides when the authored scene is ready for entry.
pub fn plugin(app: &mut App) {
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Last, (ready, computer_state, texture_progress))
        .add_systems(PreUpdate, clear_paused_input.after(bevy::input::InputSystems));
    #[cfg(not(target_arch = "wasm32"))]
    let _ = app;
}

// CODEX: publish computer focus for the browser's unobtrusive E-to-return hint.
#[cfg(target_arch = "wasm32")]
fn computer_state(
    session: Res<crate::computer::ComputerSession>,
    mut previous: Local<Option<bool>>,
) {
    let active = session.active.is_some();
    if *previous == Some(active) {
        return;
    }
    if let Some(canvas) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("lobby-canvas"))
    {
        let _ = canvas.set_attribute("data-computer-active", if active { "true" } else { "false" });
        *previous = Some(active);
    }
}

// CODEX: held movement and buttons must not survive Escape or focus loss into the next capture.
#[cfg(target_arch = "wasm32")]
fn clear_paused_input(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut motion: ResMut<bevy::input::mouse::AccumulatedMouseMotion>,
    mut was_captured: Local<bool>,
) {
    let now_captured = captured();
    // CODEX: discard the menu click and pre-capture mouse delta on the first captured frame too.
    if !now_captured || !*was_captured {
        keys.reset_all();
        buttons.reset_all();
        motion.delta = Vec2::ZERO;
    }
    *was_captured = now_captured;
}

// CODEX: both markers come from the loaded corn/glTF scene; waiting prevents premature entry.
// CLAUDE: and the low textures: the high ones keep loading after entry
#[cfg(target_arch = "wasm32")]
fn ready(
    players: Query<(), With<crate::controller::Player>>,
    monitors: Query<(), With<crate::computer::Monitor>>,
    textures: Res<texture_loading::TextureLoading>,
    mut announced: Local<bool>,
) {
    if *announced || players.is_empty() || monitors.is_empty() || !textures.low_done() {
        return;
    }
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(event) = web_sys::Event::new("corn-lobby-ready") {
            let _ = document.dispatch_event(&event);
            *announced = true;
        }
    }
}

// CLAUDE: texture loading progress for the launcher's bar and status line, as JSON in a
// `corn-lobby-progress` event's detail, when it changes
#[cfg(target_arch = "wasm32")]
fn texture_progress(textures: Res<texture_loading::TextureLoading>, mut last: Local<String>) {
    use texture_loading::LoadPhase;
    if !textures.is_changed() {
        return;
    }
    let phase = match textures.phase {
        LoadPhase::Waiting => "waiting",
        LoadPhase::Low => "low",
        LoadPhase::High => "high",
        LoadPhase::Done => "done",
        LoadPhase::Off => "off",
    };
    let progress = |p: &texture_loading::Progress| {
        serde_json::json!({
            "done": p.done, "failed": p.failed, "total": p.total, "bytes": p.bytes, "total_bytes": p.total_bytes,
        })
    };
    let json = serde_json::json!({
        "phase": phase,
        "set": textures.set,
        "current": textures.current,
        "low": progress(&textures.low),
        "high": progress(&textures.high),
    })
    .to_string();
    if *last == json {
        return;
    }
    let Some(document) = web_sys::window().and_then(|window| window.document()) else { return };
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&web_sys::wasm_bindgen::JsValue::from_str(&json));
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("corn-lobby-progress", &init) {
        let _ = document.dispatch_event(&event);
        *last = json;
    }
}
