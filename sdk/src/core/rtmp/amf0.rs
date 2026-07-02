//! Minimal AMF0 codec — just enough to build/parse the RTMP command messages
//! (`connect`, `createStream`, `publish`, `_result`, `onStatus`) the relay
//! needs. Not a general-purpose AMF0 library: unsupported markers (arrays,
//! dates, references, ...) are rejected rather than silently misparsed.

#[derive(Debug, Clone, PartialEq)]
pub enum Amf0Value {
    Number(f64),
    Boolean(bool),
    String(String),
    Object(Vec<(String, Amf0Value)>),
    Null,
    Undefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Amf0Error {
    #[error("unexpected end of AMF0 buffer")]
    UnexpectedEof,
    #[error("unsupported AMF0 type marker: 0x{0:02x}")]
    UnsupportedType(u8),
    #[error("invalid utf8 in AMF0 string")]
    InvalidUtf8,
    #[error("malformed AMF0 object")]
    Malformed,
}

impl Amf0Value {
    pub fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Amf0Value::Number(n) => {
                out.push(0x00);
                out.extend_from_slice(&n.to_be_bytes());
            }
            Amf0Value::Boolean(b) => {
                out.push(0x01);
                out.push(u8::from(*b));
            }
            Amf0Value::String(s) => {
                out.push(0x02);
                write_utf8(out, s);
            }
            Amf0Value::Object(props) => {
                out.push(0x03);
                for (key, value) in props {
                    write_utf8(out, key);
                    value.encode(out);
                }
                out.extend_from_slice(&[0x00, 0x00, 0x09]);
            }
            Amf0Value::Null => out.push(0x05),
            Amf0Value::Undefined => out.push(0x06),
        }
    }

    /// Decodes one value from the front of `buf`, returning it plus the
    /// number of bytes consumed.
    pub fn decode(buf: &[u8]) -> Result<(Amf0Value, usize), Amf0Error> {
        let marker = *buf.first().ok_or(Amf0Error::UnexpectedEof)?;
        match marker {
            0x00 => {
                let bytes = buf.get(1..9).ok_or(Amf0Error::UnexpectedEof)?;
                let mut arr = [0u8; 8];
                arr.copy_from_slice(bytes);
                Ok((Amf0Value::Number(f64::from_be_bytes(arr)), 9))
            }
            0x01 => {
                let b = *buf.get(1).ok_or(Amf0Error::UnexpectedEof)?;
                Ok((Amf0Value::Boolean(b != 0), 2))
            }
            0x02 => {
                let (s, len) = read_utf8(&buf[1..])?;
                Ok((Amf0Value::String(s), 1 + len))
            }
            0x03 => {
                let (props, len) = decode_object_body(&buf[1..])?;
                Ok((Amf0Value::Object(props), 1 + len))
            }
            0x05 => Ok((Amf0Value::Null, 1)),
            0x06 => Ok((Amf0Value::Undefined, 1)),
            other => Err(Amf0Error::UnsupportedType(other)),
        }
    }

    /// Decodes a sequential run of AMF0 values (an RTMP command payload is a
    /// concatenation of values, not wrapped in a single container).
    pub fn decode_all(mut buf: &[u8]) -> Result<Vec<Amf0Value>, Amf0Error> {
        let mut values = Vec::new();
        while !buf.is_empty() {
            let (value, len) = Amf0Value::decode(buf)?;
            values.push(value);
            buf = &buf[len..];
        }
        Ok(values)
    }
}

fn write_utf8(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(bytes);
}

fn read_utf8(buf: &[u8]) -> Result<(String, usize), Amf0Error> {
    let len_bytes = buf.get(0..2).ok_or(Amf0Error::UnexpectedEof)?;
    let len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
    let str_bytes = buf.get(2..2 + len).ok_or(Amf0Error::UnexpectedEof)?;
    let s = std::str::from_utf8(str_bytes)
        .map_err(|_| Amf0Error::InvalidUtf8)?
        .to_string();
    Ok((s, 2 + len))
}

fn decode_object_body(mut buf: &[u8]) -> Result<(Vec<(String, Amf0Value)>, usize), Amf0Error> {
    let mut props = Vec::new();
    let mut consumed = 0;
    loop {
        let (key, klen) = read_utf8(buf)?;
        buf = &buf[klen..];
        consumed += klen;
        if key.is_empty() {
            let marker = *buf.first().ok_or(Amf0Error::UnexpectedEof)?;
            if marker != 0x09 {
                return Err(Amf0Error::Malformed);
            }
            consumed += 1;
            return Ok((props, consumed));
        }
        let (value, vlen) = Amf0Value::decode(buf)?;
        buf = &buf[vlen..];
        consumed += vlen;
        props.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(value: &Amf0Value) -> Amf0Value {
        let mut buf = Vec::new();
        value.encode(&mut buf);
        let (decoded, len) = Amf0Value::decode(&buf).expect("decode");
        assert_eq!(len, buf.len());
        decoded
    }

    #[test]
    fn roundtrips_number() {
        assert_eq!(roundtrip(&Amf0Value::Number(3.5)), Amf0Value::Number(3.5));
    }

    #[test]
    fn roundtrips_boolean() {
        assert_eq!(
            roundtrip(&Amf0Value::Boolean(true)),
            Amf0Value::Boolean(true)
        );
        assert_eq!(
            roundtrip(&Amf0Value::Boolean(false)),
            Amf0Value::Boolean(false)
        );
    }

    #[test]
    fn roundtrips_string() {
        assert_eq!(
            roundtrip(&Amf0Value::String("live".into())),
            Amf0Value::String("live".into())
        );
    }

    #[test]
    fn roundtrips_null_and_undefined() {
        assert_eq!(roundtrip(&Amf0Value::Null), Amf0Value::Null);
        assert_eq!(roundtrip(&Amf0Value::Undefined), Amf0Value::Undefined);
    }

    #[test]
    fn roundtrips_nested_object() {
        let obj = Amf0Value::Object(vec![
            ("app".into(), Amf0Value::String("live".into())),
            ("capabilities".into(), Amf0Value::Number(31.0)),
            ("ok".into(), Amf0Value::Boolean(true)),
        ]);
        assert_eq!(roundtrip(&obj), obj);
    }

    #[test]
    fn decode_all_parses_command_sequence() {
        let mut buf = Vec::new();
        Amf0Value::String("connect".into()).encode(&mut buf);
        Amf0Value::Number(1.0).encode(&mut buf);
        Amf0Value::Object(vec![("app".into(), Amf0Value::String("live".into()))]).encode(&mut buf);

        let values = Amf0Value::decode_all(&buf).expect("decode_all");
        assert_eq!(values.len(), 3);
        assert_eq!(values[0], Amf0Value::String("connect".into()));
        assert_eq!(values[1], Amf0Value::Number(1.0));
    }

    #[test]
    fn decode_rejects_unsupported_marker() {
        let err = Amf0Value::decode(&[0x08, 0x00]).unwrap_err();
        assert_eq!(err, Amf0Error::UnsupportedType(0x08));
    }

    #[test]
    fn decode_rejects_truncated_buffer() {
        let err = Amf0Value::decode(&[0x00, 0x01]).unwrap_err();
        assert_eq!(err, Amf0Error::UnexpectedEof);
    }
}
