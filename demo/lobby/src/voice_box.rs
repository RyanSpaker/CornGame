//! CLAUDE (claude-opus-5-5): voice box prop on the desk. placement, prompt and sound are authored in lobby.corn.
//! E picks it up (hold.rs). Held, E "puts it in": plays `sound` (glass crack + squelch), the box is
//! despawned, and in the browser, once the sound has finished, it asks for microphone permission (the prompt
//! pauses the page, and with it the sound). It is on the desk after every load (the
//! mic must be enabled each time, even if the browser remembers the permission). If the prompt is refused or
//! the permission is later revoked, a new one is spawned from the scene file (`lobby.corn#voice_box`), where
//! it started. Native never loses the permission; scripts can say it did: `trigger(null, "MicLost")`.
use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

use bevy::{
    audio::PlaybackMode,
    prelude::*,
    world_serialization::{WorldAsset, WorldAssetRoot},
};
use corn_scripting::ScriptEventsExt;

use crate::{hold::Held, interaction::Interact};

/// CLAUDE: the voice box's own scene: `spawn voice_box` in lobby.corn
const SCENE: &str = "lobby.corn#voice_box";

/// CLAUDE: [`SCENE`], held from the start (see `keep_scene`)
#[derive(Resource)]
struct VoiceBoxScene(Handle<WorldAsset>);

/// CLAUDE: on the voice box's sound: when it is gone (played out, `PlaybackMode::Despawn`), ask for the mic
#[derive(Component)]
struct MicAfterSound;

// CLAUDE
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct VoiceBox {
    pub sound: Handle<AudioSource>,
}

/// CLAUDE: the microphone permission was refused or revoked (browser callbacks; scripts, for testing)
#[derive(Event, Reflect, Clone, Debug, Default)]
pub struct MicLost;

// CLAUDE: written by browser callbacks, read each frame. Native never leaves Requested after use.
static MIC: AtomicU8 = AtomicU8::new(UNKNOWN);
const UNKNOWN: u8 = 0;
const REQUESTED: u8 = 1;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const GRANTED: u8 = 2;
const DENIED: u8 = 3;

// CODEX (GPT-6): an optional real capture backend replaces the offline permission-only probe.
#[derive(Resource, Default)]
pub struct MicrophoneBackend { pub external: bool }

// CODEX: the consuming adapter starts media only after the local box's sound has finished.
pub fn microphone_active() -> bool { MIC.load(Relaxed) == GRANTED }

pub fn plugin(app: &mut App) {
    app.init_resource::<MicrophoneBackend>()
        .register_type::<VoiceBox>()
        .register_script_event::<MicLost>()
        .add_observer(interact)
        .add_observer(request_after_sound)
        .add_observer(|_: On<MicLost>| MIC.store(DENIED, Relaxed))
        .add_systems(Startup, keep_scene.before(crate::load))
        .add_systems(Update, respawn_without_mic);
    #[cfg(target_arch = "wasm32")]
    app.add_systems(Startup, watch_permission);
}

// CLAUDE: only while held (unheld, E picks it up). The sound is its own entity, where the box was, so it
// plays on after the box is gone.
fn interact(
    on: On<Interact>,
    boxes: Query<(&VoiceBox, &GlobalTransform), With<Held>>,
    mut commands: Commands,
) {
    let Ok((voice_box, at)) = boxes.get(on.entity) else {
        return;
    };
    commands.spawn((
        MicAfterSound,
        Transform::from_translation(at.translation()),
        AudioPlayer(voice_box.sound.clone()),
        PlaybackSettings {
            mode: PlaybackMode::Despawn,
            spatial: true,
            ..default()
        },
    ));
    commands.entity(on.entity).despawn();
    MIC.store(REQUESTED, Relaxed);
    info!(entity = %on.entity, "voice box put in, microphone requested");
}

// CLAUDE: the sound has finished: now the browser's prompt may pause the page
fn request_after_sound(_: On<Remove<MicAfterSound>>, backend: Res<MicrophoneBackend>) {
    if backend.external { MIC.store(GRANTED, Relaxed); return; }
    #[cfg(target_arch = "wasm32")]
    request_microphone();
}

// CLAUDE: ask for SCENE before the lobby's own load (main.rs) asks for lobby.corn: the labeled path starts that
// one load, which lobby.corn's then joins. The other way round lobby.corn is loaded twice, and asked for only
// once needed, it's gone and lobby.corn loads (and the lobby respawns) again
fn keep_scene(assets: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(VoiceBoxScene(assets.load(SCENE)));
}

// CLAUDE: the permission is lost: a new box where it started, unless one is still there (refused before the
// box was used: nothing to do). Unknown again, so it's spawned once.
fn respawn_without_mic(boxes: Query<(), With<VoiceBox>>, scene: Res<VoiceBoxScene>, mut commands: Commands) {
    if MIC.load(Relaxed) != DENIED {
        return;
    }
    MIC.store(UNKNOWN, Relaxed);
    if boxes.is_empty() {
        info!("microphone permission lost: the voice box is back");
        commands.spawn((Name::new("voice box (respawned)"), WorldAssetRoot(scene.0.clone())));
    }
}

/// CLAUDE: shows the browser's mic permission prompt (once per origin; later calls resolve immediately).
/// needs a secure context (https or localhost), otherwise `navigator.mediaDevices` is undefined.
#[cfg(target_arch = "wasm32")]
fn request_microphone() {
    use web_sys::wasm_bindgen::JsCast;

    wasm_bindgen_futures::spawn_local(async {
        let Some(devices) = web_sys::window().and_then(|w| w.navigator().media_devices().ok()) else {
            warn!("navigator.mediaDevices unavailable (not a secure context?)");
            MIC.store(DENIED, Relaxed);
            return;
        };
        let constraints = web_sys::MediaStreamConstraints::new();
        constraints.set_audio(&true.into());
        let promise = match devices.get_user_media_with_constraints(&constraints) {
            Ok(promise) => promise,
            Err(e) => {
                warn!(?e, "getUserMedia failed");
                MIC.store(DENIED, Relaxed);
                return;
            }
        };
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(stream) => {
                info!("microphone permission granted");
                MIC.store(GRANTED, Relaxed);
                // nothing consumes the stream yet; stop it so the browser's recording indicator goes away
                let stream: web_sys::MediaStream = stream.unchecked_into();
                for track in stream.get_tracks().iter() {
                    track.unchecked_into::<web_sys::MediaStreamTrack>().stop();
                }
            }
            Err(e) => {
                warn!(?e, "microphone permission denied");
                MIC.store(DENIED, Relaxed);
            }
        }
    });
}

/// CLAUDE: notice the mic permission being revoked (e.g. in site settings) via `navigator.permissions`.
/// Only losses count: a remembered or newly granted permission doesn't hide the box, using it does.
/// Browsers without a queryable "microphone" permission only get the getUserMedia result.
#[cfg(target_arch = "wasm32")]
fn watch_permission() {
    use web_sys::{
        js_sys,
        wasm_bindgen::{closure::Closure, JsCast},
        PermissionState, PermissionStatus,
    };

    fn on_change(state: PermissionState) {
        if state != PermissionState::Granted {
            MIC.store(DENIED, Relaxed);
        }
    }

    wasm_bindgen_futures::spawn_local(async {
        let Some(permissions) = web_sys::window().and_then(|w| w.navigator().permissions().ok()) else {
            return;
        };
        let descriptor = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&descriptor, &"name".into(), &"microphone".into());
        let Ok(promise) = permissions.query(&descriptor) else {
            return;
        };
        let status: PermissionStatus = match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(status) => status.unchecked_into(),
            Err(e) => {
                warn!(?e, "can't watch microphone permission");
                return;
            }
        };
        // the closure owns a reference to the status, keeping it (and its onchange) alive
        let watched = status.clone();
        let callback = Closure::<dyn FnMut()>::new(move || on_change(watched.state()));
        status.set_onchange(Some(callback.as_ref().unchecked_ref()));
        callback.forget();
    });
}
