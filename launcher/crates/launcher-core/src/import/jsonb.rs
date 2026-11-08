//! SQLite's binary JSON (JSONB), as the Modrinth App stores its JSON columns
//! (`jsonb(?)`). Format: <https://sqlite.org/jsonb.html>. Each element is a header byte
//! (type in the low four bits, payload size or size-of-size in the high four), an
//! optional big-endian size, and the payload; numbers and strings keep their JSON text,
//! arrays and objects hold their elements one after another.

use serde_json::{Map, Number, Value};

const MAX_DEPTH: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid JSONB: {0}")]
pub struct JsonbError(String);

fn invalid(what: &str) -> JsonbError {
    JsonbError(what.to_string())
}

pub fn decode(bytes: &[u8]) -> Result<Value, JsonbError> {
    let (value, end) = element(bytes, 0, 0)?;
    if end != bytes.len() {
        return Err(invalid("trailing bytes"));
    }
    Ok(value)
}

/// The element at `at`, and where the next one starts.
fn element(bytes: &[u8], at: usize, depth: usize) -> Result<(Value, usize), JsonbError> {
    if depth > MAX_DEPTH {
        return Err(invalid("too deeply nested"));
    }
    let header = *bytes.get(at).ok_or_else(|| invalid("truncated"))?;
    let kind = header & 0x0f;
    let (size, header_len) = match header >> 4 {
        size @ 0..=11 => (size as usize, 1),
        code => {
            let n = 1usize << (code - 12);
            let raw = bytes
                .get(at + 1..at + 1 + n)
                .ok_or_else(|| invalid("truncated size"))?;
            let size = raw.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64);
            (usize::try_from(size).map_err(|_| invalid("size"))?, 1 + n)
        }
    };
    let start = at + header_len;
    let end = start.checked_add(size).ok_or_else(|| invalid("size"))?;
    let payload = bytes
        .get(start..end)
        .ok_or_else(|| invalid("truncated payload"))?;
    let text = || std::str::from_utf8(payload).map_err(|_| invalid("text is not UTF-8"));
    let value = match kind {
        0 => Value::Null,
        1 => Value::Bool(true),
        2 => Value::Bool(false),
        3 | 5 => serde_json::from_str::<Number>(text()?)
            .map(Value::Number)
            .map_err(|_| invalid("number"))?,
        4 | 6 => {
            // JSON5 numbers: hex integers, a leading '+', '.5'.
            let raw = text()?.trim_start_matches('+');
            let (negative, raw) = match raw.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, raw),
            };
            if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
                let value = i64::from_str_radix(hex, 16).map_err(|_| invalid("number"))?;
                Value::from(if negative { -value } else { value })
            } else {
                let value: f64 = raw.parse().map_err(|_| invalid("number"))?;
                Number::from_f64(if negative { -value } else { value })
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
        }
        7 | 10 => Value::String(text()?.to_string()),
        8 | 9 => serde_json::from_str::<String>(&format!("\"{}\"", text()?))
            .map(Value::String)
            .map_err(|_| invalid("escaped text"))?,
        11 => {
            let mut items = Vec::new();
            let mut at = start;
            while at < end {
                let (item, next) = element(&bytes[..end], at, depth + 1)?;
                items.push(item);
                at = next;
            }
            Value::Array(items)
        }
        12 => {
            let mut map = Map::new();
            let mut at = start;
            while at < end {
                let (key, next) = element(&bytes[..end], at, depth + 1)?;
                let Value::String(key) = key else {
                    return Err(invalid("object key is not text"));
                };
                let (value, next) = element(&bytes[..end], next, depth + 1)?;
                map.insert(key, value);
                at = next;
            }
            Value::Object(map)
        }
        _ => return Err(invalid("reserved element type")),
    };
    Ok((value, end))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn decodes_elements() {
        // ["-Xmx2G", 3, true, null]
        let mut array = vec![0x67];
        array.extend(b"-Xmx2G");
        array.extend([0x13, b'3', 0x01, 0x00]);
        let mut bytes = vec![0xcb, array.len() as u8];
        bytes.extend(&array);
        assert_eq!(decode(&bytes).unwrap(), json!(["-Xmx2G", 3, true, null]));
        // {"maximum": 4096}
        let mut object = vec![0x77];
        object.extend(b"maximum");
        object.extend([0x43]);
        object.extend(b"4096");
        let mut bytes = vec![0xcc, object.len() as u8];
        bytes.extend(&object);
        assert_eq!(decode(&bytes).unwrap(), json!({"maximum": 4096}));
        // TEXTJ with an escape
        let mut bytes = vec![0x68];
        bytes.extend(b"a\\\"b\\n");
        assert_eq!(decode(&bytes).unwrap(), json!("a\"b\n"));
        assert_eq!(decode(&[0x34, b'0', b'x', b'f']).unwrap(), json!(15));
        assert_eq!(
            decode(&[0x14, b'0', b'x']).unwrap_err(),
            invalid("trailing bytes")
        );
        assert!(decode(&[0x77, b'a']).is_err());
        assert!(decode(&[0x0d]).is_err());
    }
}
