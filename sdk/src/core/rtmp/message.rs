//! RTMP command message builders/parser (AMF0 payloads) and the message-type
//! / chunk-stream-id constants the relay uses.

use super::amf0::{Amf0Error, Amf0Value};

pub const MSG_TYPE_AUDIO: u8 = 8;
pub const MSG_TYPE_VIDEO: u8 = 9;
pub const MSG_TYPE_AMF0_DATA: u8 = 18;
pub const MSG_TYPE_AMF0_COMMAND: u8 = 20;

/// Dedicated chunk stream ids, mirroring common RTMP muxer convention.
pub const CSID_COMMAND: u32 = 3;
pub const CSID_VIDEO: u32 = 4;
pub const CSID_AUDIO: u32 = 5;
pub const CSID_DATA: u32 = 6;

fn encode_values(values: &[Amf0Value]) -> Vec<u8> {
    let mut out = Vec::new();
    for v in values {
        v.encode(&mut out);
    }
    out
}

pub fn connect_command(app: &str, tc_url: &str) -> Vec<u8> {
    encode_values(&[
        Amf0Value::String("connect".into()),
        Amf0Value::Number(1.0),
        Amf0Value::Object(vec![
            ("app".into(), Amf0Value::String(app.into())),
            ("type".into(), Amf0Value::String("nonprivate".into())),
            (
                "flashVer".into(),
                Amf0Value::String("FMLE/3.0 (compatible; StreamDubber)".into()),
            ),
            ("tcUrl".into(), Amf0Value::String(tc_url.into())),
        ]),
    ])
}

pub fn create_stream_command(transaction_id: f64) -> Vec<u8> {
    encode_values(&[
        Amf0Value::String("createStream".into()),
        Amf0Value::Number(transaction_id),
        Amf0Value::Null,
    ])
}

pub fn publish_command(stream_key: &str) -> Vec<u8> {
    encode_values(&[
        Amf0Value::String("publish".into()),
        Amf0Value::Number(0.0),
        Amf0Value::Null,
        Amf0Value::String(stream_key.into()),
        Amf0Value::String("live".into()),
    ])
}

pub fn connect_result(transaction_id: f64) -> Vec<u8> {
    encode_values(&[
        Amf0Value::String("_result".into()),
        Amf0Value::Number(transaction_id),
        Amf0Value::Object(vec![
            ("fmsVer".into(), Amf0Value::String("FMS/3,5,7,7009".into())),
            ("capabilities".into(), Amf0Value::Number(31.0)),
        ]),
        Amf0Value::Object(vec![
            ("level".into(), Amf0Value::String("status".into())),
            (
                "code".into(),
                Amf0Value::String("NetConnection.Connect.Success".into()),
            ),
            (
                "description".into(),
                Amf0Value::String("Connection succeeded.".into()),
            ),
        ]),
    ])
}

pub fn create_stream_result(transaction_id: f64, stream_id: f64) -> Vec<u8> {
    encode_values(&[
        Amf0Value::String("_result".into()),
        Amf0Value::Number(transaction_id),
        Amf0Value::Null,
        Amf0Value::Number(stream_id),
    ])
}

pub fn publish_start_status() -> Vec<u8> {
    encode_values(&[
        Amf0Value::String("onStatus".into()),
        Amf0Value::Number(0.0),
        Amf0Value::Null,
        Amf0Value::Object(vec![
            ("level".into(), Amf0Value::String("status".into())),
            (
                "code".into(),
                Amf0Value::String("NetStream.Publish.Start".into()),
            ),
            ("description".into(), Amf0Value::String("Publishing".into())),
        ]),
    ])
}

/// A decoded RTMP command: `[String(name), Number(transaction_id), ...args]`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCommand {
    pub name: String,
    pub transaction_id: f64,
    pub args: Vec<Amf0Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    #[error("amf0 decode error: {0}")]
    Amf(Amf0Error),
    #[error("command payload does not start with a name string")]
    MissingName,
}

impl From<Amf0Error> for CommandError {
    fn from(e: Amf0Error) -> Self {
        CommandError::Amf(e)
    }
}

pub fn parse_command(payload: &[u8]) -> Result<ParsedCommand, CommandError> {
    let values = Amf0Value::decode_all(payload)?;
    let mut iter = values.into_iter();
    let name = match iter.next() {
        Some(Amf0Value::String(s)) => s,
        _ => return Err(CommandError::MissingName),
    };
    let transaction_id = match iter.next() {
        Some(Amf0Value::Number(n)) => n,
        _ => 0.0,
    };
    Ok(ParsedCommand {
        name,
        transaction_id,
        args: iter.collect(),
    })
}

/// `true` if an `onStatus`/`_result` reply's info object has `level: "error"`.
pub fn reply_signals_error(cmd: &ParsedCommand) -> bool {
    cmd.args.iter().any(|v| {
        if let Amf0Value::Object(props) = v {
            props
                .iter()
                .any(|(k, v)| k == "level" && matches!(v, Amf0Value::String(s) if s == "error"))
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_command_round_trip() {
        let payload = connect_command("live2", "rtmp://a.rtmp.youtube.com/live2");
        let cmd = parse_command(&payload).expect("parse");
        assert_eq!(cmd.name, "connect");
        assert_eq!(cmd.transaction_id, 1.0);
        match &cmd.args[0] {
            Amf0Value::Object(props) => {
                assert!(props
                    .iter()
                    .any(|(k, v)| k == "app" && *v == Amf0Value::String("live2".into())));
                assert!(props.iter().any(|(k, v)| k == "tcUrl"
                    && *v == Amf0Value::String("rtmp://a.rtmp.youtube.com/live2".into())));
            }
            other => panic!("expected object, got {other:?}"),
        }
    }

    #[test]
    fn parses_publish_command_stream_key() {
        let payload = publish_command("sk-secret-123");
        let cmd = parse_command(&payload).expect("parse");
        assert_eq!(cmd.name, "publish");
        assert_eq!(cmd.args[1], Amf0Value::String("sk-secret-123".into()));
    }

    #[test]
    fn parses_create_stream_result_id() {
        let payload = create_stream_result(2.0, 5.0);
        let cmd = parse_command(&payload).expect("parse");
        assert_eq!(cmd.name, "_result");
        assert_eq!(cmd.args[1], Amf0Value::Number(5.0));
    }

    #[test]
    fn detects_error_level_in_reply() {
        let payload = encode_values(&[
            Amf0Value::String("onStatus".into()),
            Amf0Value::Number(0.0),
            Amf0Value::Null,
            Amf0Value::Object(vec![
                ("level".into(), Amf0Value::String("error".into())),
                (
                    "code".into(),
                    Amf0Value::String("NetStream.Publish.BadName".into()),
                ),
            ]),
        ]);
        let cmd = parse_command(&payload).expect("parse");
        assert!(reply_signals_error(&cmd));
    }

    #[test]
    fn publish_start_status_is_not_an_error() {
        let cmd = parse_command(&publish_start_status()).expect("parse");
        assert!(!reply_signals_error(&cmd));
    }
}
