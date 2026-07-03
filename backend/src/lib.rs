//! StreamDubber backend — kütüphane kökü.
//!
//! Faz 1: WebSocket ses pipeline, API gateway, auth. Bu crate hem binary'yi
//! hem de testleri besler.

pub mod app;
pub mod auth;
pub mod billing;
pub mod config;
pub mod crypto;
pub mod db;
pub mod gateway;
pub mod integrations;
pub mod logging;
pub mod overlay;
pub mod pipeline;
pub mod plans;
pub mod provider_check;
pub mod secrets;
pub mod state;
pub mod tls;
pub mod usage;
pub mod ws;
