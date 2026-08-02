//! The VELT daemon: a loopback-only sidecar.
//!
//! Doctrine §5: *Private deal flow. Nothing leaves the machine without an
//! explicit, visible, user-initiated action.* The listener is bound to
//! `127.0.0.1` unconditionally — the bind address is not configurable, because a
//! config option that can expose an operator's deal pipeline to their LAN is a
//! footgun with no upside.

#![forbid(unsafe_code)]

mod api;

use anyhow::Context as _;
use std::{net::SocketAddr, sync::Arc};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi as _;

/// Loopback address. Not configurable, by design.
const BIND_ADDR: [u8; 4] = [127, 0, 0, 1];

/// Default port; overridable via `VELT_PORT`. Port 0 asks the OS for a free port,
/// which is how the Electron shell avoids colliding with a stale daemon.
const DEFAULT_PORT: u16 = 47_821;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("VELT_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // `--openapi` emits the contract and exits; this is what `just openapi`
    // calls, so the checked-in spec can never drift from the code that serves it.
    if std::env::args().any(|a| a == "--openapi") {
        println!("{}", api::ApiDoc::openapi().to_pretty_json()?);
        return Ok(());
    }

    let port = std::env::var("VELT_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let state = Arc::new(api::AppState {
        engine_version: env!("CARGO_PKG_VERSION"),
    });
    let app = api::router(state).layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from((BIND_ADDR, port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding loopback {addr}"))?;
    let bound = listener.local_addr()?;

    // The shell parses this line to discover the port. Keep the format stable.
    println!("VELT_DAEMON_LISTENING {bound}");
    tracing::info!(%bound, "velt daemon listening on loopback");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving")?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
