//! Embedded RTMP relay engine (Faz 1.6) — accepts the broadcaster's local OBS
//! publish and forwards audio/video to N destination platforms concurrently,
//! each using its own stream key. Video never leaves the broadcaster's own
//! internet connection; no cloud relay server is needed.
//!
//! Scope note: this module owns the wire protocol (handshake, AMF0, chunk
//! stream) and the relay orchestration. Binding an actual TCP listener for
//! OBS to connect to, and pulling the encrypted target list from the web
//! panel, are desktop-shell wiring done where the SDK is assembled — same
//! split as `pairing`, whose backend endpoint landed separately.

mod amf0;
mod chunk;
mod handshake;
mod message;
mod relay;
mod session;
mod target;

pub use relay::{RelayEngine, TargetState, TargetStatus};
pub use session::RtmpError;
pub use target::{RelayTarget, TargetError};
