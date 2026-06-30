//! Web panel pairing — links this SDK install to a broadcaster account.
//!
//! Flow: dashboard shows a 6-digit code → broadcaster types it into the SDK →
//! SDK POSTs the code + device info to the backend → backend marks the
//! `sdk_sessions` row paired. The backend pairing endpoint lands in Phase 1.4;
//! this module owns the SDK-side validation and request payload.

use serde::Serialize;

/// Required pairing code length (see `sdk_sessions.pairing_code`).
pub const CODE_LEN: usize = 6;

/// Pairing validation errors.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PairingError {
    #[error("pairing code must be {CODE_LEN} digits")]
    BadLength,
    #[error("pairing code must be numeric")]
    NotNumeric,
}

/// A validated 6-digit pairing code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    /// Parses and validates a pairing code (trims surrounding whitespace).
    pub fn parse(input: &str) -> Result<Self, PairingError> {
        let trimmed = input.trim();
        if trimmed.len() != CODE_LEN {
            return Err(PairingError::BadLength);
        }
        if !trimmed.bytes().all(|b| b.is_ascii_digit()) {
            return Err(PairingError::NotNumeric);
        }
        Ok(PairingCode(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Payload sent to the backend pairing endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairingRequest {
    pub pairing_code: String,
    pub device_name: String,
    pub sdk_version: String,
}

impl PairingRequest {
    pub fn new(code: &PairingCode, device_name: impl Into<String>) -> Self {
        PairingRequest {
            pairing_code: code.as_str().to_string(),
            device_name: device_name.into(),
            sdk_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_six_digit_code() {
        let code = PairingCode::parse(" 123456 ").expect("valid");
        assert_eq!(code.as_str(), "123456");
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(PairingCode::parse("123"), Err(PairingError::BadLength));
        assert_eq!(PairingCode::parse("1234567"), Err(PairingError::BadLength));
    }

    #[test]
    fn rejects_non_numeric() {
        assert_eq!(PairingCode::parse("12ab56"), Err(PairingError::NotNumeric));
    }

    #[test]
    fn request_carries_sdk_version() {
        let code = PairingCode::parse("654321").unwrap();
        let req = PairingRequest::new(&code, "Windows PC");
        assert_eq!(req.pairing_code, "654321");
        assert_eq!(req.device_name, "Windows PC");
        assert!(!req.sdk_version.is_empty());
    }
}
