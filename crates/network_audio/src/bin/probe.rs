//! CODEX (GPT-6): standalone microphone/tone client, live JSON diagnostics, and stdin runtime controls.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use network_audio::{VoiceClient, VoiceConfig, VoiceMode, Pose};
    use std::io::BufRead;
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if args.iter().any(|a| a == "--help") {
        println!("voice-probe [--url ws://127.0.0.1:8787/voice] [--room lobby] [--mode peer_to_peer|server] [--tone 440] [--headless] [--seconds 30] [--input-device ID] [--output-device ID] [--list-devices] [--x 2] [--y 0] [--z 0]\nstdin JSON commands: {{\"mode\":\"server\"}}, {{\"pose\":{{\"position\":[2,0,0],\"forward\":[0,0,-1],\"up\":[0,1,0]}}}}, {{\"muted\":true}}, {{\"spatial\":{{\"volume\":1,\"referenceDistance\":1,\"maxDistance\":30,\"rolloff\":1}}}}, {{\"stop\":true}}");
        return Ok(());
    }
    // CODEX: listing device IDs never starts capture or joins a voice room.
    if args.iter().any(|a| a == "--list-devices") {
        use cpal::traits::{HostTrait, DeviceTrait};
        for device in cpal::default_host().devices()? {
            if let Ok(id) = device.id() {
                println!(
                    "{}\t{}",
                    id,
                    device
                        .description()
                        .map(|d| d.to_string())
                        .unwrap_or_default()
                );
            }
        }
        return Ok(());
    }
    let mut config = VoiceConfig {
        input_device: value("--input-device"),
        output_device: value("--output-device"),
        ..Default::default()
    };
    if let Some(url) = value("--url") {
        config.signaling_url = url;
    }
    if let Some(room) = value("--room") {
        config.room = room;
    }
    if let Some(mode) = value("--mode") {
        config.mode = serde_json::from_value(serde_json::Value::String(mode))?;
    }
    config.test_tone_hz = value("--tone").map(|v| v.parse()).transpose()?;
    config.headless = args.iter().any(|a| a == "--headless");
    if let Ok(json) = std::env::var("VOICE_ICE_SERVERS") {
        config.ice_servers = serde_json::from_str(&json)?;
    }
    let position = [value("--x"), value("--y"), value("--z")]
        .map(|v| v.and_then(|x| x.parse().ok()).unwrap_or(0.));
    let seconds = value("--seconds")
        .map(|v| v.parse::<u64>())
        .transpose()?
        .unwrap_or(86_400);
    let mut client = VoiceClient::start(config).map_err(anyhow::Error::msg)?;
    client.set_pose(Pose {
        position,
        ..Default::default()
    });
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < seconds {
        for line in rx.try_iter() {
            let json: serde_json::Value = serde_json::from_str(&line)?;
            if let Some(mode) = json.get("mode") {
                client
                    .set_mode(serde_json::from_value::<VoiceMode>(mode.clone())?)
                    .map_err(anyhow::Error::msg)?;
            }
            if let Some(pose) = json.get("pose") {
                client.set_pose(serde_json::from_value(pose.clone())?);
            }
            if let Some(muted) = json.get("muted").and_then(|x| x.as_bool()) {
                client.set_muted(muted);
            }
            if let Some(spatial) = json.get("spatial") {
                client.set_spatial(serde_json::from_value(spatial.clone())?);
            }
            if json.get("stop").and_then(|v| v.as_bool()) == Some(true) {
                return Ok(());
            }
        }
        let status = client.status();
        println!("{}", serde_json::to_string(&status)?);
        if let Some(error) = status.error {
            anyhow::bail!(error);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(())
}
// CODEX: probe CLI owns native audio devices; browser demo is served by voice-relay.
#[cfg(target_arch = "wasm32")]
fn main() {}
