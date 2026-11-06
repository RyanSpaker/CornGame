//! CODEX (GPT-6): shared native/SFU WebRTC setup. Register only Opus so the native decoder is unambiguous.
use std::sync::Arc;
use webrtc::{
    api::{
        APIBuilder,
        media_engine::{MediaEngine, MIME_TYPE_OPUS},
        interceptor_registry::register_default_interceptors,
    },
    interceptor::registry::Registry,
    peer_connection::{RTCPeerConnection, configuration::RTCConfiguration},
    ice_transport::ice_server::RTCIceServer,
    rtp_transceiver::{
        rtp_codec::{RTCRtpCodecCapability, RTCRtpCodecParameters, RTPCodecType},
        rtp_sender::RTCRtpSender,
    },
};
use crate::IceServer;

// CODEX: browsers negotiate Opus as two-channel RTP even when the microphone payload is mono.
pub fn opus_capability() -> RTCRtpCodecCapability {
    RTCRtpCodecCapability {
        mime_type: MIME_TYPE_OPUS.into(),
        clock_rate: 48_000,
        channels: 2,
        sdp_fmtp_line: "minptime=10;useinbandfec=1".into(),
        ..Default::default()
    }
}

// CODEX: interceptors handle RTCP reports; ICE accepts caller-provided STUN/TURN configuration.
pub async fn connection(servers: &[IceServer]) -> anyhow::Result<Arc<RTCPeerConnection>> {
    let mut media = MediaEngine::default();
    media.register_codec(
        RTCRtpCodecParameters {
            capability: opus_capability(),
            payload_type: 111,
            ..Default::default()
        },
        RTPCodecType::Audio,
    )?;
    let registry = register_default_interceptors(Registry::new(), &mut media)?;
    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();
    Ok(Arc::new(
        api.new_peer_connection(RTCConfiguration {
            ice_servers: servers
                .iter()
                .map(|s| RTCIceServer {
                    urls: s.urls.clone(),
                    username: s.username.clone(),
                    credential: s.credential.clone(),
                })
                .collect(),
            ..Default::default()
        })
        .await?,
    ))
}

// CODEX: continuously drain feedback so RTCP backpressure cannot stall outgoing media.
pub fn drain_rtcp(sender: Arc<RTCRtpSender>) {
    tokio::spawn(async move {
        let mut buffer = [0u8; 1500];
        while sender.read(&mut buffer).await.is_ok() {}
    });
}
