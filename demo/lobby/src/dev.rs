//! CLAUDE (claude-opus-5-5): what scripts need to drive the lobby as a player would, through corn_scripting's
//! remote console (`CORN_DEV=<port>`, then `corn-dev`, see .agents/doc/remote_console.md) or the smoke
//! test (`LOBBY_SMOKE=<dir>`: runs assets/dev/smoke.js and exits, failing on the first failed check).
//!
//! Input goes through the same `ButtonInput` the game reads, fed after the real input each frame:
//! `trigger(null, "Press", { key: "KeyE" })` (down one frame), `trigger(null, "Hold", { key: "KeyW", down: true })`,
//! `trigger(null, "Click", { button: "Left", down: true })`. `resource("NoMouse").on = true` leaves the real
//! mouse alone: no cursor grabbing, no mouse look.
//! CODEX (GPT-6): relative pointer/text injection supports the isolated dummy walkthrough script.
use bevy::{
    input::{mouse::AccumulatedMouseMotion, InputSystems},
    prelude::*,
};
use corn_scripting::{RunScript, ScriptEventsExt};

use crate::controller::NoMouse;

/// CLAUDE: press `key` for one frame
#[derive(Event, Reflect, Clone, Debug)]
pub struct Press {
    pub key: KeyCode,
}

/// CLAUDE: hold `key` down (or let it go)
#[derive(Event, Reflect, Clone, Debug)]
pub struct Hold {
    pub key: KeyCode,
    pub down: bool,
}

/// CLAUDE: press or release a mouse button
#[derive(Event, Reflect, Clone, Debug)]
pub struct Click {
    pub button: MouseButton,
    pub down: bool,
}

// CODEX (GPT-6): one-frame relative pointer motion, consumed by the existing computer/radio controls.
#[derive(Event, Reflect, Clone, Debug)]
pub struct MoveMouse { pub delta: Vec2 }

// CODEX (GPT-6): text goes through the real paper editor, including its word filter and length limit.
#[derive(Event, Reflect, Clone, Debug)]
pub struct TypeText { pub text: String }

/// CLAUDE: a smoke check failed: exit with an error
#[derive(Event, Reflect, Clone, Debug, Default)]
pub struct SmokeFailed {
    pub message: String,
}

/// CLAUDE: input from scripts, applied after the real input next frame
#[derive(Resource, Default)]
struct Scripted {
    /// `Press`: down this frame, up the next
    press: Vec<KeyCode>,
    release: Vec<KeyCode>,
    /// `Hold`
    keys: Vec<(KeyCode, bool)>,
    buttons: Vec<(MouseButton, bool)>,
    // CODEX: reset each frame after applying, including when physical mouse input is suppressed.
    motion: Vec2,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Scripted>()
        .init_resource::<NoMouse>()
        .register_type::<NoMouse>()
        .register_type::<crate::interaction::InteractionPrompt>()
        .register_type::<crate::computer::ComputerSession>()
        .register_script_event::<Press>()
        .register_script_event::<Hold>()
        .register_script_event::<Click>()
        .register_script_event::<MoveMouse>()
        .register_script_event::<TypeText>()
        .register_script_event::<SmokeFailed>()
        .add_observer(|on: On<Press>, mut input: ResMut<Scripted>| input.press.push(on.event().key))
        .add_observer(|on: On<Hold>, mut input: ResMut<Scripted>| input.keys.push((on.event().key, on.event().down)))
        .add_observer(|on: On<Click>, mut input: ResMut<Scripted>| input.buttons.push((on.event().button, on.event().down)))
        // CODEX: input adapters only; the dummy's behavior lives entirely in tools/dummy_player.py.
        .add_observer(|on: On<MoveMouse>, mut input: ResMut<Scripted>| input.motion += on.event().delta)
        .add_observer(|on: On<TypeText>, mut text: MessageWriter<bevy::input::keyboard::KeyboardInput>, window: Single<Entity, With<bevy::window::PrimaryWindow>>| {
            use bevy::input::{keyboard::{Key, KeyboardInput, NativeKeyCode}, ButtonState};
            for character in on.event().text.chars() {
                let value = character.to_string();
                text.write(KeyboardInput { key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
                    logical_key: Key::Character(value.clone().into()), text: Some(value.into()),
                    state: ButtonState::Pressed, repeat: false, window: *window });
            }
        })
        .add_observer(|on: On<SmokeFailed>, mut exit: MessageWriter<AppExit>| {
            error!("LOBBY_SMOKE failed: {}", on.event().message);
            exit.write(AppExit::error());
        })
        // before the monitor reads the mouse buttons (computer.rs)
        .add_systems(PreUpdate, apply.after(InputSystems).before(crate::computer::pointer_input));
    if let Ok(dir) = std::env::var("LOBBY_SMOKE") {
        std::fs::create_dir_all(&dir).expect("create LOBBY_SMOKE output directory");
        let from = std::env::var("LOBBY_SMOKE_FROM").unwrap_or_default();
        // CODEX — GPT-6.1: Focused feature smoke scripts can reuse the existing input/screenshot harness.
        let script = std::env::var("LOBBY_SMOKE_SCRIPT").unwrap_or_else(|_| "dev/smoke.js".into());
        // the smoke runs while its entity lives
        app.add_systems(Startup, move |mut commands: Commands| {
            let smoke = commands.spawn(Name::new("smoke")).id();
            let args = serde_json::json!({ "dir": dir, "from": from });
            commands.queue(RunScript::new(script.clone(), "smoke", smoke).with_args(args));
        });
    }
}

// CLAUDE
fn apply(
    mut input: ResMut<Scripted>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    no_mouse: Res<NoMouse>,
    mut motion: ResMut<AccumulatedMouseMotion>,
) {
    if no_mouse.on {
        motion.delta = Vec2::ZERO;
    }
    let Scripted { press, release, keys: held, buttons: clicks, motion: scripted_motion } = &mut *input;
    // CODEX: scripted motion is applied once after NoMouse clears physical input.
    motion.delta += std::mem::take(scripted_motion);
    for key in release.drain(..) {
        keys.release(key);
    }
    for key in press.drain(..) {
        keys.press(key);
        release.push(key);
    }
    for (key, down) in held.drain(..) {
        if down {
            keys.press(key);
        } else {
            keys.release(key);
        }
    }
    for (button, down) in clicks.drain(..) {
        if down {
            buttons.press(button);
        } else {
            buttons.release(button);
        }
    }
}
