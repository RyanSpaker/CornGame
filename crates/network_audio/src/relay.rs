//! CODEX (GPT-6): embeddable signaling + Opus SFU. Mount VoiceRelay::router() in the future game server.
//! No listener, process runtime, game replication, or authentication policy is owned by this module.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use axum::{
    Router,
    extract::{
        State,
        WebSocketUpgrade,
        ws::{WebSocket, Message},
    },
    response::IntoResponse,
    routing::get,
};
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use webrtc::{
    peer_connection::{RTCPeerConnection, sdp::session_description::RTCSessionDescription},
    track::track_local::{
        track_local_static_rtp::TrackLocalStaticRTP,
        TrackLocal,
        TrackLocalWriter,
    },
};
use crate::{rtc, spatial, IceServer, Signal, VoiceMode};

/// CODEX: fixed receive slots avoid renegotiating every existing connection on a room join/leave.
/// A later protocol revision can replace this demo-sized bound with negotiated/dynamic subscriptions.
pub const ROOM_CAPACITY: usize = 8;
type RoomKey = (String, VoiceMode);

/// CODEX: caller owns authentication (apply an axum layer) and the HTTP/TLS listener/runtime.
#[derive(Clone, Default)]
pub struct VoiceRelay {
    inner: Arc<Mutex<Rooms>>,
    ice_servers: Arc<Vec<IceServer>>,
}
type Rooms = HashMap<RoomKey, HashMap<String, Member>>;
// CODEX: bounded channels prevent signaling backpressure from blocking an entire room.
struct Member {
    tx: mpsc::Sender<Signal>,
    pc: Option<Arc<RTCPeerConnection>>,
    tracks: HashMap<String, Arc<ForwardTrack>>,
}

// CODEX: a reserved SFU slot keeps its outgoing RTP clock continuous when a new speaker reuses it.
struct ForwardTrack {
    track: Arc<TrackLocalStaticRTP>,
    clock: Mutex<Option<RtpClock>>,
}

// CODEX: record both source and forwarded RTP origins so reused slots retain a continuous clock.
#[derive(Clone, Copy)]
struct RtpClock {
    source_ssrc: u32,
    source_sequence: u16,
    source_timestamp: u32,
    sequence: u16,
    timestamp: u32,
}

// CODEX: preserve normal packet loss/timing deltas, but rebase a replacement source's random RTP origin.
impl ForwardTrack {
    // CODEX: Keep per-recipient RTP sequence and timestamp continuity across speaker-slot reuse.
    async fn write(&self, packet: &webrtc::rtp::packet::Packet) -> webrtc::error::Result<usize> {
        let mut forwarded = packet.clone();
        {
            let mut clock = self.clock.lock().unwrap();
            let input = &packet.header;
            let (sequence, timestamp) = if let Some(previous) = *clock {
                let delta_seq = input.sequence_number.wrapping_sub(previous.source_sequence);
                let delta_ts = input.timestamp.wrapping_sub(previous.source_timestamp);
                if previous.source_ssrc == input.ssrc
                    && delta_seq > 0
                    && delta_seq < 64
                    && delta_ts <= 48_000
                {
                    (
                        previous.sequence.wrapping_add(delta_seq),
                        previous.timestamp.wrapping_add(delta_ts),
                    )
                } else if previous.source_ssrc == input.ssrc && delta_seq >= 32768 {
                    // CODEX: late packets cannot move the rewritten sequence/timestamp backwards.
                    return Ok(0);
                } else {
                    (
                        previous.sequence.wrapping_add(1),
                        previous.timestamp.wrapping_add(960),
                    )
                }
            } else {
                (input.sequence_number, input.timestamp)
            };
            *clock = Some(RtpClock {
                source_ssrc: input.ssrc,
                source_sequence: input.sequence_number,
                source_timestamp: input.timestamp,
                sequence,
                timestamp,
            });
            forwarded.header.sequence_number = sequence;
            forwarded.header.timestamp = timestamp;
        }
        self.track.write_rtp(&forwarded).await
    }
}

// CODEX: reusable service with no process-global state; independent instances can be mounted in tests.
impl VoiceRelay {
    // CODEX: Create isolated service or mixer state without opening any network listener.
    pub fn new(ice_servers: Vec<IceServer>) -> Self {
        Self {
            ice_servers: Arc::new(ice_servers),
            ..Default::default()
        }
    }
    // CODEX: Expose embeddable voice routes while leaving listener and middleware ownership to the host.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/voice", get(upgrade))
            .with_state(self.clone())
    }
    // CODEX: the SFU forwards compressed Opus without decoding/mixing; endpoints retain speaker identity.
    async fn media_peer(
        &self,
        key: &RoomKey,
        id: &str,
        out: mpsc::Sender<Signal>,
    ) -> anyhow::Result<Arc<RTCPeerConnection>> {
        let pc = rtc::connection(&self.ice_servers).await?;
        let mut tracks = HashMap::new();
        for slot in 0..ROOM_CAPACITY {
            let source = slot.to_string();
            if source == id {
                continue;
            }
            let track = Arc::new(TrackLocalStaticRTP::new(
                rtc::opus_capability(),
                format!("voice-{source}"),
                format!("speaker-{source}"),
            ));
            rtc::drain_rtcp(
                pc.add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>)
                    .await?,
            );
            tracks.insert(
                source,
                Arc::new(ForwardTrack {
                    track,
                    clock: Mutex::new(None),
                }),
            );
        }
        {
            let mut rooms = self.inner.lock().unwrap();
            let member = rooms
                .get_mut(key)
                .and_then(|r| r.get_mut(id))
                .ok_or_else(|| anyhow::anyhow!("Member left during setup"))?;
            member.pc = Some(pc.clone());
            member.tracks = tracks;
        }
        pc.on_ice_candidate(Box::new(move |candidate| {
            let out = out.clone();
            Box::pin(async move {
                if let Some(candidate) = candidate {
                    if let Ok(candidate) = candidate.to_json() {
                        let _ = out
                            .send(Signal::Candidate {
                                to: String::new(),
                                from: "server".into(),
                                candidate: serde_json::to_value(candidate).unwrap(),
                            })
                            .await;
                    }
                }
            })
        }));
        let relay = self.clone();
        let room = key.clone();
        let source = id.to_string();
        pc.on_track(Box::new(move |track, _, _| {
            let relay = relay.clone();
            let room = room.clone();
            let source = source.clone();
            // CODEX: track readers must be independent of WebRTC's on_track dispatch future.
            tokio::spawn(async move {
                while let Ok((packet, _)) = track.read_rtp().await {
                    let targets: Vec<_> = {
                        let rooms = relay.inner.lock().unwrap();
                        rooms
                            .get(&room)
                            .map(|members| {
                                members
                                    .values()
                                    .filter_map(|m| m.tracks.get(&source).cloned())
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    for target in targets {
                        let _ = target.write(&packet).await;
                    }
                }
            });
            Box::pin(async {})
        }));
        Ok(pc)
    }
    // CODEX: never await under the room registry lock; periodic metadata may drop under backpressure.
    fn broadcast(&self, key: &RoomKey, except: &str, signal: Signal) {
        if let Some(room) = self.inner.lock().unwrap().get(key) {
            for (id, member) in room {
                if id != except {
                    let _ = member.tx.try_send(signal.clone());
                }
            }
        }
    }
}

// CODEX: host can mount this router under a prefix; client config supplies the resulting URL.
async fn upgrade(State(relay): State<VoiceRelay>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.max_message_size(65_536)
        .max_frame_size(65_536)
        .on_upgrade(move |socket| session(relay, socket))
}

// CODEX: accept one room join per socket, stamp sender identity server-side, and isolate room/mode pairs.
async fn session(relay: VoiceRelay, mut socket: WebSocket) {
    let first = tokio::time::timeout(std::time::Duration::from_secs(10), socket.next()).await;
    let Some(Signal::Join { room, mode }) =
        first
            .ok()
            .flatten()
            .and_then(Result::ok)
            .and_then(|m| match m {
                Message::Text(t) => serde_json::from_str(&t).ok(),
                _ => None,
            })
    else {
        return;
    };
    if room.is_empty()
        || room.len() > 64
        || !room
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
    {
        return;
    }
    let key = (room, mode);
    let (out, mut outgoing) = mpsc::channel::<Signal>(256);
    let registered = {
        let mut rooms = relay.inner.lock().unwrap();
        let members = rooms.entry(key.clone()).or_default();
        if let Some(id) = (0..ROOM_CAPACITY)
            .map(|n| n.to_string())
            .find(|n| !members.contains_key(n))
        {
            let peers = members.keys().cloned().collect::<Vec<_>>();
            members.insert(
                id.clone(),
                Member {
                    tx: out.clone(),
                    pc: None,
                    tracks: HashMap::new(),
                },
            );
            Some((id, peers))
        } else {
            None
        }
    };
    let Some((id, peers)) = registered else {
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&Signal::Error {
                    message: format!("Voice room is full ({ROOM_CAPACITY})"),
                })
                .unwrap()
                .into(),
            ))
            .await;
        return;
    };
    let (mut sink, mut incoming) = socket.split();
    let writer = tokio::spawn(async move {
        while let Some(signal) = outgoing.recv().await {
            if sink
                .send(Message::Text(
                    serde_json::to_string(&signal).unwrap().into(),
                ))
                .await
                .is_err()
            {
                break;
            }
        }
    });
    let _ = out
        .send(Signal::Welcome {
            id: id.clone(),
            peers: if mode == VoiceMode::PeerToPeer {
                peers
            } else {
                vec![]
            },
        })
        .await;
    let result: anyhow::Result<()> = async {
        let pc = if mode==VoiceMode::Server {
            let pc=relay.media_peer(&key,&id,out.clone()).await?;
            let offer=pc.create_offer(None).await?; pc.set_local_description(offer.clone()).await?;
            out.send(Signal::Description {to:id.clone(),from:"server".into(),description:serde_json::to_value(offer)?}).await?;
            Some(pc)
        } else {None};
        let mut pending=Vec::new();
        loop {
            let message = tokio::select! {
                message=incoming.next()=>message,
                // CODEX: end the read side if the socket writer has failed, ensuring member cleanup.
                _=tokio::time::sleep(std::time::Duration::from_secs(5))=>{ if writer.is_finished() { break; } else {continue;} },
            };
            let Some(message)=message else {break;};
            let Message::Text(text)=message? else {continue;};
            let signal: Signal=serde_json::from_str(&text)?;
            match signal {
                Signal::Pose {pose,..} if spatial::valid_pose(pose)=>relay.broadcast(&key,&id,Signal::Pose {from:id.clone(),pose}),
                Signal::Description {to,description,..}=>{
                    if let Some(pc)=&pc {
                        let description:RTCSessionDescription=serde_json::from_value(description)?;
                        if description.sdp_type!=webrtc::peer_connection::sdp::sdp_type::RTCSdpType::Answer { anyhow::bail!("Server expects an SDP answer"); }
                        pc.set_remote_description(description).await?;
                        for candidate in pending.drain(..) {pc.add_ice_candidate(candidate).await?;}
                    } else {
                        let target=relay.inner.lock().unwrap().get(&key).and_then(|r|r.get(&to)).map(|m|m.tx.clone());
                        if let Some(tx)=target {tx.try_send(Signal::Description {to,from:id.clone(),description})?;}
                    }
                },
                Signal::Candidate {to,candidate,..}=>{
                    if let Some(pc)=&pc {
                        let candidate=serde_json::from_value(candidate)?;
                        if pc.remote_description().await.is_some() {pc.add_ice_candidate(candidate).await?;}
                        else if pending.len()<64 {pending.push(candidate);}
                    } else {
                        let target=relay.inner.lock().unwrap().get(&key).and_then(|r|r.get(&to)).map(|m|m.tx.clone());
                        if let Some(tx)=target {tx.try_send(Signal::Candidate {to,from:id.clone(),candidate})?;}
                    }
                },
                _=>{},
            }
        }
        Ok(())
    }.await;
    if let Err(error) = result {
        let _ = out
            .send(Signal::Error {
                message: error.to_string(),
            })
            .await;
    }
    let member = {
        let mut rooms = relay.inner.lock().unwrap();
        let member = rooms.get_mut(&key).and_then(|r| r.remove(&id));
        if rooms.get(&key).is_some_and(|r| r.is_empty()) {
            rooms.remove(&key);
        }
        member
    };
    if let Some(pc) = member.and_then(|m| m.pc) {
        let _ = pc.close().await;
    }
    relay.broadcast(&key, &id, Signal::Left { id: id.clone() });
    writer.abort();
}
