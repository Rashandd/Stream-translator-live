//! StreamDubber backend — kütüphane kökü.
//!
//! Faz 1: WebSocket ses pipeline, API gateway, auth. Bu crate hem binary'yi
//! hem de testleri besler.

pub mod app;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod gateway;
pub mod integrations;
pub mod pipeline;
pub mod secrets;
pub mod state;
pub mod tls;
pub mod ws;
