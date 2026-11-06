//! CODEX (GPT-6): standalone host for the embeddable service and browser test page.
#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::{
        routing::get,
        response::{Html, IntoResponse},
        http::header,
    };
    let servers = std::env::var("VOICE_ICE_SERVERS")
        .ok()
        .map(|s| serde_json::from_str(&s))
        .transpose()?
        .unwrap_or_default();
    let relay = network_audio::relay::VoiceRelay::new(servers);
    let app = relay
        .router()
        .route(
            "/",
            get(|| async { Html(include_str!("../../web/index.html")) }),
        )
        .route(
            "/voice.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/voice.js"),
                )
                    .into_response()
            }),
        )
        .route(
            "/demo.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript")],
                    include_str!("../../web/demo.js"),
                )
                    .into_response()
            }),
        );
    let address = std::env::var("VOICE_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!(
        "Voice test: http://{} (P2P signaling + server-routed Opus)",
        listener.local_addr()?
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
// CODEX: server processes are native-only; keep workspace wasm checks possible.
#[cfg(target_arch = "wasm32")]
fn main() {}
