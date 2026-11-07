//! CODEX (GPT-6): browser/native WebRTC voice. Positions use Bevy's metres, +Y up, -Z forward.
//! Signaling carries room membership and poses only; audio travels over encrypted WebRTC media.
use serde::{Deserialize, Serialize};

#[cfg(all(not(target_arch = "wasm32"), feature = "client"))]
mod audio;
#[cfg(all(target_arch = "wasm32", feature = "client"))]
mod browser;
#[cfg(all(not(target_arch = "wasm32"), feature = "client"))]
mod device;
#[cfg(all(not(target_arch = "wasm32"), feature = "client"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "relay"))]
pub mod relay;
#[cfg(not(target_arch = "wasm32"))]
pub mod rtc;
pub mod spatial;
#[cfg(all(not(target_arch = "wasm32"), feature = "client"))]
pub use native::VoiceClient;
#[cfg(all(target_arch = "wasm32", feature = "client"))]
pub use browser::VoiceClient;

/// CODEX: keep ICE credentials configurable; use short-lived TURN credentials in deployed clients.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub credential: String,
}

/// CODEX: shared controls have identical semantics in native and browser playback.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceConfig {
    pub signaling_url: String,
    pub room: String,
    pub mode: VoiceMode,
    pub ice_servers: Vec<IceServer>,
    pub spatial: spatial::SpatialSettings,
    /// A test tone bypasses capture on both platforms. Never enabled by the lobby by default.
    pub test_tone_hz: Option<f32>,
    /// Native integration probes mix audio without opening a physical output device.
    pub headless: bool,
    /// CODEX: native device ID/name override; Linux additionally accepts an ALSA PCM expression.
    pub input_device: Option<String>,
    /// CODEX: e.g. pipewire:NODE=<sink name>; browser capture/playback ignores native overrides.
    pub output_device: Option<String>,
    /// CLAUDE: capture from the start. false listens only until `VoiceClient::set_microphone(true)`,
    /// which swaps the microphone in without reconnecting (incoming speech keeps playing).
    pub microphone: bool,
}

// CODEX: local/LAN works without a third-party service; internet hosts supply STUN/TURN explicitly.
impl Default for VoiceConfig {
    // CODEX: Supply metre-scale defaults shared by native and browser clients.
    fn default() -> Self {
        Self {
            signaling_url: "ws://127.0.0.1:8787/voice".into(),
            room: "lobby".into(),
            mode: VoiceMode::PeerToPeer,
            ice_servers: vec![],
            spatial: spatial::SpatialSettings::default(),
            test_tone_hz: None,
            headless: false,
            input_device: None,
            output_device: None,
            microphone: true,
        }
    }
}

// CODEX: reject malformed local configuration before opening devices or asking for microphone access.
impl VoiceConfig {
    // CODEX: Reject invalid configuration before activating capture.
    pub fn validate(&self) -> Result<(), String> {
        if !self.signaling_url.starts_with("ws://") && !self.signaling_url.starts_with("wss://") {
            return Err("signaling_url must use ws:// or wss://".into());
        }
        if self.room.is_empty()
            || self.room.len() > 64
            || !self
                .room
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
        {
            return Err("room must contain 1–64 ASCII letters, digits, '-' or '_'".into());
        }
        if self
            .test_tone_hz
            .is_some_and(|hz| !hz.is_finite() || !(20.0..=20_000.0).contains(&hz))
        {
            return Err("test tone must be between 20 and 20000 Hz".into());
        }
        Ok(())
    }
}

/// CODEX: topology is a room choice; switching rejoins the same room in the selected topology.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceMode {
    #[default]
    PeerToPeer,
    Server,
}

/// CODEX: listener orientation is orthonormalized by the mixer; source position is the same player's head.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub up: [f32; 3],
}

// CODEX: match Bevy's default transform basis.
impl Default for Pose {
    // CODEX: Supply metre-scale defaults shared by native and browser clients.
    fn default() -> Self {
        Self {
            position: [0.; 3],
            forward: [0., 0., -1.],
            up: [0., 1., 0.],
        }
    }
}

/// CODEX: read-only diagnostics are suitable for UI and automated acoustic verification.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VoiceStatus {
    pub state: String,
    pub error: Option<String>,
    pub peers: usize,
    /// CLAUDE: WebRTC connections currently up (P2P: one per peer; server mode: 1 for the relay)
    pub connected: usize,
    pub received_packets: u64,
    pub sent_packets: u64,
    pub input_level: f32,
    pub output_left: f32,
    pub output_right: f32,
    pub muted: bool,
    /// CLAUDE: the microphone is capturing (a test tone counts)
    pub microphone: bool,
    /// CLAUDE: the microphone failed to open or was lost; receiving is unaffected. Cleared by set_microphone.
    pub microphone_error: Option<String>,
}

/// CODEX: small JSON signaling protocol shared with web/voice.js and the optional relay binary.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Signal {
    Join {
        room: String,
        #[serde(default)]
        mode: VoiceMode,
    },
    Welcome {
        id: String,
        peers: Vec<String>,
    },
    Description {
        to: String,
        #[serde(default)]
        from: String,
        description: serde_json::Value,
    },
    Candidate {
        to: String,
        #[serde(default)]
        from: String,
        candidate: serde_json::Value,
    },
    Pose {
        #[serde(default)]
        from: String,
        pose: Pose,
    },
    Left {
        id: String,
    },
    Error {
        message: String,
    },
}
