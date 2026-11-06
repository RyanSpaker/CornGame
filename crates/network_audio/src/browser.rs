//! CODEX (GPT-6): thin wasm-bindgen adapter; all browser media ownership stays in web/voice.js.
use wasm_bindgen::prelude::*;
use crate::{Pose, VoiceConfig, VoiceMode, VoiceStatus, spatial::SpatialSettings};

// CODEX: wasm-bindgen copies this module into its output snippets, including standalone lobby builds later.
#[wasm_bindgen(module = "/web/voice.js")]
extern "C" {
    #[wasm_bindgen(js_name = createVoice)]
    // CODEX: Construct the browser-owned media session from serialized shared configuration.
    fn create_voice(config: &str) -> BrowserVoice;
    type BrowserVoice;
    #[wasm_bindgen(method, js_name = setPose)]
    // CODEX: Update the listener and microphone pose without waiting for network work.
    fn set_pose(this: &BrowserVoice, json: &str);
    #[wasm_bindgen(method, js_name = setSpatial)]
    // CODEX: Apply live playback tuning using the shared finite-value limits.
    fn set_spatial(this: &BrowserVoice, json: &str);
    #[wasm_bindgen(method, js_name = setMuted)]
    // CODEX: Keep the connection alive while replacing outgoing speech with silence.
    fn set_muted(this: &BrowserVoice, muted: bool);
    #[wasm_bindgen(method)]
    // CODEX: Read browser status without exposing media objects to the game engine.
    fn snapshot(this: &BrowserVoice) -> String;
    #[wasm_bindgen(method)]
    // CODEX: Match the cross-platform API; browser audio may require a fresh input gesture.
    fn resume(this: &BrowserVoice);
    #[wasm_bindgen(method, js_name = switchMode)]
    // CODEX: Rejoin the selected topology while preserving local audio settings.
    fn switch_mode(this: &BrowserVoice, mode: &str) -> BrowserVoice;
    #[wasm_bindgen(method)]
    // CODEX: Release browser media tracks, audio nodes and sockets.
    fn close(this: &BrowserVoice);
}

/// CODEX: insert as a NonSend Bevy resource when integrating; JS handles belong to the browser thread.
pub struct VoiceClient(BrowserVoice);

// CODEX: public API matches native; start must be invoked from a real browser input gesture.
impl VoiceClient {
    // CODEX: Start platform capture and transport; status reports asynchronous failures.
    pub fn start(config: VoiceConfig) -> Result<Self, String> {
        config.validate()?;
        Ok(Self(create_voice(
            &serde_json::to_string(&config).map_err(|e| e.to_string())?,
        )))
    }
    // CODEX: Update the listener and microphone pose without waiting for network work.
    pub fn set_pose(&self, pose: Pose) {
        if crate::spatial::valid_pose(pose) {
            self.0.set_pose(&serde_json::to_string(&pose).unwrap());
        }
    }
    // CODEX: Apply live playback tuning using the shared finite-value limits.
    pub fn set_spatial(&self, settings: SpatialSettings) {
        self.0
            .set_spatial(&serde_json::to_string(&crate::spatial::sanitize(settings)).unwrap());
    }
    // CODEX: Keep the connection alive while replacing outgoing speech with silence.
    pub fn set_muted(&self, muted: bool) {
        self.0.set_muted(muted);
    }
    // CODEX: Return a snapshot suitable for UI and acoustic verification.
    pub fn status(&self) -> VoiceStatus {
        serde_json::from_str(&self.0.snapshot()).unwrap_or_default()
    }
    // CODEX: Restart transport in the selected topology while retaining pose and mute.
    pub fn set_mode(&mut self, mode: VoiceMode) -> Result<(), String> {
        self.0 = self.0.switch_mode(match mode {
            VoiceMode::PeerToPeer => "peer_to_peer",
            VoiceMode::Server => "server",
        });
        Ok(())
    }
    // CODEX: Match the cross-platform API; browser audio may require a fresh input gesture.
    pub fn resume(&self) {
        self.0.resume();
    }
}

// CODEX: deterministic teardown prevents retained recording indicators and orphan WebRTC sessions.
impl Drop for VoiceClient {
    // CODEX: Stop media ownership deterministically when the client leaves scope.
    fn drop(&mut self) {
        self.0.close();
    }
}
