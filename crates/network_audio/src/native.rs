//! CODEX (GPT-6): native WebRTC worker. CPAL, Opus and sockets never run on the game's update thread.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use webrtc::{
    peer_connection::{RTCPeerConnection, sdp::session_description::RTCSessionDescription},
    ice_transport::ice_candidate::RTCIceCandidateInit,
    track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
    media::Sample,
};
use crate::{
    audio::{Devices, State, RATE, FRAME},
    rtc,
    spatial::{self, SpatialSettings},
    Pose,
    Signal,
    VoiceConfig,
    VoiceMode,
    VoiceStatus,
};

/// CODEX: Send-safe handle; synchronous setters only update small shared state, never await networking.
pub struct VoiceClient {
    shared: Arc<Mutex<State>>,
    stop: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

// CODEX: transport errors are asynchronous and available through status(), including device failures.
impl VoiceClient {
    // CODEX: Start platform capture and transport; status reports asynchronous failures.
    pub fn start(config: VoiceConfig) -> Result<Self, String> {
        config.validate()?;
        let shared = Arc::new(Mutex::new(State::new(config)));
        let state = shared.clone();
        let (stop, shutdown) = oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("network-audio".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        state.lock().unwrap().error(e);
                        return;
                    }
                };
                if let Err(error) = runtime.block_on(run(state.clone(), shutdown)) {
                    state.lock().unwrap().error(format!("{error:#}"));
                }
                runtime.shutdown_timeout(Duration::from_secs(2));
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            shared,
            stop: Some(stop),
            worker: Some(worker),
        })
    }
    // CODEX: Update the listener and microphone pose without waiting for network work.
    pub fn set_pose(&self, pose: Pose) {
        if spatial::valid_pose(pose) {
            self.shared.lock().unwrap().pose = pose;
        }
    }
    // CODEX: Apply live playback tuning using the shared finite-value limits.
    pub fn set_spatial(&self, settings: SpatialSettings) {
        self.shared.lock().unwrap().config.spatial = spatial::sanitize(settings);
    }
    // CODEX: Keep the connection alive while replacing outgoing speech with silence.
    pub fn set_muted(&self, muted: bool) {
        self.shared.lock().unwrap().status.muted = muted;
    }
    // CODEX: Return a snapshot suitable for UI and acoustic verification.
    pub fn status(&self) -> VoiceStatus {
        self.shared.lock().unwrap().status.clone()
    }
    // CODEX: Match the cross-platform API; browser audio may require a fresh input gesture.
    pub fn resume(&self) {}
    // CODEX: switching closes the old transport/devices first and preserves pose, mute and live tuning.
    pub fn set_mode(&mut self, mode: VoiceMode) -> Result<(), String> {
        let (mut config, pose, muted) = {
            let s = self.shared.lock().unwrap();
            (s.config.clone(), s.pose, s.status.muted)
        };
        if config.mode == mode {
            return Ok(());
        }
        config.mode = mode;
        self.shutdown();
        *self = Self::start(config)?;
        self.set_pose(pose);
        self.set_muted(muted);
        Ok(())
    }
    // CODEX: Release the active worker and devices before a runtime topology change.
    fn shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
// CODEX: join the worker so no stale connection or capture stream survives a mode change or drop.
impl Drop for VoiceClient {
    // CODEX: Stop media ownership deterministically when the client leaves scope.
    fn drop(&mut self) {
        self.shutdown();
    }
}

// CODEX: one outgoing track can fan out to multiple peer connections without re-encoding each copy.
struct Peer {
    pc: Arc<RTCPeerConnection>,
    pending: Vec<RTCIceCandidateInit>,
}

// CODEX: a bounded socket writer decouples ICE callbacks from the signaling socket.
async fn run(state: Arc<Mutex<State>>, mut shutdown: oneshot::Receiver<()>) -> anyhow::Result<()> {
    let config = state.lock().unwrap().config.clone();
    // CODEX: opening devices before signaling ensures unusable clients never appear ready in a room.
    let _devices = Devices::open(state.clone())?;
    let (socket, _) = tokio::select! {
        result = tokio_tungstenite::connect_async(&config.signaling_url) => result?,
        _ = &mut shutdown => return Ok(()),
    };
    let (mut sink, mut incoming) = socket.split();
    let (out, mut outgoing) = mpsc::channel::<Signal>(256);
    out.send(Signal::Join {
        room: config.room.clone(),
        mode: config.mode,
    })
    .await?;
    let writer = tokio::spawn(async move {
        while let Some(signal) = outgoing.recv().await {
            sink.send(Message::Text(serde_json::to_string(&signal)?.into()))
                .await?;
        }
        sink.close().await?;
        anyhow::Ok(())
    });
    let track = Arc::new(TrackLocalStaticSample::new(
        rtc::opus_capability(),
        "microphone".into(),
        "voice".into(),
    ));
    let mut peers: HashMap<String, Peer> = HashMap::new();
    let mut orphan_candidates: HashMap<String, Vec<RTCIceCandidateInit>> = HashMap::new();
    let mut encoder =
        opus::Encoder::new(RATE as u32, opus::Channels::Mono, opus::Application::Voip)?;
    encoder.set_bitrate(opus::Bitrate::Bits(32_000))?;
    let mut tick = tokio::time::interval(Duration::from_millis(20));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut count = 0u64;
    let mut phase = 0f32;
    let result: anyhow::Result<()> = async {
        loop {
            tokio::select! {
                _ = &mut shutdown => break,
                _ = tick.tick() => {
                    let (frame, pose) = {
                        let mut s = state.lock().unwrap();
                        if let Some(error) = &s.status.error { anyhow::bail!(error.clone()); }
                        let mut frame = [0f32; FRAME];
                        if let Some(hz) = config.test_tone_hz {
                            for x in &mut frame { *x = phase.sin()*0.15; phase = (phase+std::f32::consts::TAU*hz/RATE as f32) % std::f32::consts::TAU; }
                        } else {
                            let available = FRAME.min(s.capture.len());
                            for (sample, value) in frame.iter_mut().zip(s.capture.drain(..available)) { *sample = value; }
                        }
                        if s.status.muted { frame.fill(0.); }
                        s.status.input_level = (frame.iter().map(|x|x*x).sum::<f32>()/FRAME as f32).sqrt();
                        if config.headless { let mut output = [0f32; FRAME*2]; s.mix(&mut output, RATE as u32); }
                        (frame, s.pose)
                    };
                    let mut packet = [0u8; 4000]; let size = encoder.encode_float(&frame, &mut packet)?;
                    track.write_sample(&Sample { data:bytes::Bytes::copy_from_slice(&packet[..size]), duration:Duration::from_millis(20), ..Default::default() }).await?;
                    state.lock().unwrap().status.sent_packets += 1;
                    count += 1;
                    if count.is_multiple_of(3) { out.try_send(Signal::Pose { from:String::new(), pose })?; }
                    if writer.is_finished() { anyhow::bail!("Voice signaling writer stopped"); }
                },
                message = incoming.next() => {
                    let message = message.ok_or_else(|| anyhow::anyhow!("Voice signaling disconnected"))??;
                    let Message::Text(text) = message else { continue; };
                    match serde_json::from_str::<Signal>(&text)? {
                        Signal::Welcome { peers:ids, .. } => {
                            state.lock().unwrap().status.state = "ready".into();
                            for id in ids {
                                let peer = make_peer(&id, &config, state.clone(), track.clone(), out.clone()).await?;
                                let offer = peer.pc.create_offer(None).await?;
                                peer.pc.set_local_description(offer.clone()).await?;
                                out.send(Signal::Description { to:id.clone(), from:String::new(), description:serde_json::to_value(offer)? }).await?;
                                peers.insert(id, peer);
                            }
                        },
                        Signal::Description { from, description, .. } => {
                            if !peers.contains_key(&from) {
                                let mut peer = make_peer(&from, &config, state.clone(), track.clone(), out.clone()).await?;
                                peer.pending = orphan_candidates.remove(&from).unwrap_or_default();
                                peers.insert(from.clone(), peer);
                            }
                            let peer = peers.get_mut(&from).unwrap();
                            let description: RTCSessionDescription = serde_json::from_value(description)?;
                            let offer = description.sdp_type == webrtc::peer_connection::sdp::sdp_type::RTCSdpType::Offer;
                            peer.pc.set_remote_description(description).await?;
                            for candidate in peer.pending.drain(..) { peer.pc.add_ice_candidate(candidate).await?; }
                            if offer {
                                let answer = peer.pc.create_answer(None).await?;
                                peer.pc.set_local_description(answer.clone()).await?;
                                out.send(Signal::Description { to:from, from:String::new(), description:serde_json::to_value(answer)? }).await?;
                            }
                        },
                        Signal::Candidate { from, candidate, .. } => {
                            let candidate: RTCIceCandidateInit = serde_json::from_value(candidate)?;
                            if let Some(peer) = peers.get_mut(&from) {
                                if peer.pc.remote_description().await.is_some() { peer.pc.add_ice_candidate(candidate).await?; }
                                else if peer.pending.len() < 64 { peer.pending.push(candidate); }
                            } else if orphan_candidates.len() < 16 {
                                let pending = orphan_candidates.entry(from).or_default();
                                if pending.len() < 64 { pending.push(candidate); }
                            }
                        },
                        Signal::Pose { from, pose } => {
                            if spatial::valid_pose(pose) { state.lock().unwrap().speakers.entry(from).or_default().position = Some(pose.position); }
                        },
                        Signal::Left { id } => {
                            if let Some(peer) = peers.remove(&id) { peer.pc.close().await?; }
                            orphan_candidates.remove(&id); state.lock().unwrap().speakers.remove(&id);
                        },
                        Signal::Error { message } => anyhow::bail!(message),
                        _ => {},
                    }
                    { let mut s = state.lock().unwrap(); s.status.peers = s.speakers.len(); }
                }
            }
        }
        Ok(())
    }.await;
    for peer in peers.values() {
        let _ = peer.pc.close().await;
    }
    writer.abort();
    {
        let mut s = state.lock().unwrap();
        s.speakers.clear();
        s.status.peers = 0;
        if result.is_ok() {
            s.status.state = "stopped".into();
        }
    }
    result
}

// CODEX: server mode multiplexes identifiable speaker tracks over one connection, P2P uses one per peer.
async fn make_peer(
    id: &str,
    config: &VoiceConfig,
    state: Arc<Mutex<State>>,
    track: Arc<TrackLocalStaticSample>,
    out: mpsc::Sender<Signal>,
) -> anyhow::Result<Peer> {
    let pc = rtc::connection(&config.ice_servers).await?;
    let sender = pc
        .add_track(track as Arc<dyn TrackLocal + Send + Sync>)
        .await?;
    rtc::drain_rtcp(sender);
    let to = id.to_string();
    pc.on_ice_candidate(Box::new(move |candidate| {
        let out = out.clone();
        let to = to.clone();
        Box::pin(async move {
            if let Some(candidate) = candidate {
                if let Ok(candidate) = candidate.to_json() {
                    let _ = out
                        .send(Signal::Candidate {
                            to,
                            from: String::new(),
                            candidate: serde_json::to_value(candidate).unwrap(),
                        })
                        .await;
                }
            }
        })
    }));
    let remote_id = id.to_string();
    let server = config.mode == VoiceMode::Server;
    pc.on_track(Box::new(move |track, _, _| {
        let state = state.clone();
        let id = if server {
            track
                .id()
                .strip_prefix("voice-")
                .unwrap_or(&track.id())
                .to_string()
        } else {
            remote_id.clone()
        };
        tokio::spawn(async move {
            let (packets, receiver) = mpsc::channel(64);
            let decoder = tokio::spawn(decode(id, state, receiver));
            while let Ok((packet, _)) = track.read_rtp().await {
                if packets.send(packet).await.is_err() {
                    break;
                }
            }
            decoder.abort();
        });
        Box::pin(async {})
    }));
    Ok(Peer {
        pc,
        pending: vec![],
    })
}

// CODEX: reorder a bounded RTP window, tolerate sequence wrap, and conceal losses on a playout clock.
async fn decode(
    id: String,
    state: Arc<Mutex<State>>,
    mut incoming: mpsc::Receiver<webrtc::rtp::packet::Packet>,
) {
    let Ok(mut decoder) = opus::Decoder::new(RATE as u32, opus::Channels::Mono) else {
        return;
    };
    let Some(first) = incoming.recv().await else {
        return;
    };
    let mut expected = first.header.sequence_number;
    let mut packets = HashMap::new();
    packets.insert(expected, first);
    let mut deadline = tokio::time::Instant::now() + Duration::from_millis(40);
    let mut pcm = [0f32; 5760];
    let mut last_accepted = tokio::time::Instant::now();
    loop {
        tokio::select! {
            next = incoming.recv() => {
                let Some(packet) = next else { break; };
                let now = tokio::time::Instant::now();
                let delta = packet.header.sequence_number.wrapping_sub(expected);
                // CODEX: a reserved SFU track can resume with a new source after a silent slot interval.
                if now.duration_since(last_accepted) > Duration::from_millis(250) {
                    packets.clear(); expected = packet.header.sequence_number;
                    let _ = decoder.reset_state(); deadline = now+Duration::from_millis(40);
                    last_accepted = now; packets.insert(expected, packet);
                } else if delta < 64 {
                    last_accepted = now; packets.insert(packet.header.sequence_number, packet);
                } else if delta < 32768 {
                    last_accepted = now; packets.clear(); expected=packet.header.sequence_number; packets.insert(expected,packet);
                }
            },
            _ = tokio::time::sleep_until(deadline) => {
                let packet = packets.remove(&expected); expected=expected.wrapping_add(1);
                let payload = packet.as_ref().map(|p| p.payload.as_ref()).unwrap_or(&[]);
                let buffer = if packet.is_some() { &mut pcm[..] } else { &mut pcm[..FRAME] };
                match decoder.decode_float(payload, buffer, false) {
                    Ok(samples) => {
                        let mut s=state.lock().unwrap();
                        // CODEX: do not resurrect a departed speaker while its final read task shuts down.
                        if s.speakers.contains_key(&id) {
                            s.push_pcm(&id, &pcm[..samples]);
                            if packet.is_some() { s.status.received_packets += 1; }
                        }
                        deadline += Duration::from_secs_f64(samples as f64/RATE as f64);
                    },
                    Err(_) => { deadline += Duration::from_millis(20); }
                }
                if deadline < tokio::time::Instant::now()-Duration::from_millis(100) { deadline=tokio::time::Instant::now(); }
            }
        }
    }
}
