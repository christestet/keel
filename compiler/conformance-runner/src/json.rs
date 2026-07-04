//! Minimal JSON reader for the `image` runner mode, which must parse back the
//! OCI layout `keelc build --image` writes (spec ch19) to check it structurally.
//! Hand-rolled, not a dependency: this crate is zero-dependency by design
//! (see Cargo.toml) and only ever reads JSON this same toolchain produced, so
//! only the subset needed to navigate it is implemented.

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
    Other,
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n) => Some(*n as u64),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    let value = parse_value(bytes, &mut pos)?;
    Ok(value)
}

fn skip_ws(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

fn parse_value(bytes: &[u8], pos: &mut usize) -> Result<Value, String> {
    skip_ws(bytes, pos);
    match bytes.get(*pos) {
        Some(b'{') => parse_object(bytes, pos),
        Some(b'[') => parse_array(bytes, pos),
        Some(b'"') => parse_string(bytes, pos).map(Value::String),
        Some(b't') => {
            expect_literal(bytes, pos, "true")?;
            Ok(Value::Other)
        }
        Some(b'f') => {
            expect_literal(bytes, pos, "false")?;
            Ok(Value::Other)
        }
        Some(b'n') => {
            expect_literal(bytes, pos, "null")?;
            Ok(Value::Other)
        }
        Some(_) => parse_number(bytes, pos),
        None => Err("unexpected end of JSON".into()),
    }
}

fn expect_literal(bytes: &[u8], pos: &mut usize, lit: &str) -> Result<(), String> {
    let end = *pos + lit.len();
    if bytes.get(*pos..end) == Some(lit.as_bytes()) {
        *pos = end;
        Ok(())
    } else {
        Err(format!("expected `{lit}`"))
    }
}

fn parse_object(bytes: &[u8], pos: &mut usize) -> Result<Value, String> {
    *pos += 1; // '{'
    let mut entries = Vec::new();
    skip_ws(bytes, pos);
    if bytes.get(*pos) == Some(&b'}') {
        *pos += 1;
        return Ok(Value::Object(entries));
    }
    loop {
        skip_ws(bytes, pos);
        let key = parse_string(bytes, pos)?;
        skip_ws(bytes, pos);
        if bytes.get(*pos) != Some(&b':') {
            return Err("expected `:` in object".into());
        }
        *pos += 1;
        let value = parse_value(bytes, pos)?;
        entries.push((key, value));
        skip_ws(bytes, pos);
        match bytes.get(*pos) {
            Some(b',') => {
                *pos += 1;
            }
            Some(b'}') => {
                *pos += 1;
                break;
            }
            _ => return Err("expected `,` or `}` in object".into()),
        }
    }
    Ok(Value::Object(entries))
}

fn parse_array(bytes: &[u8], pos: &mut usize) -> Result<Value, String> {
    *pos += 1; // '['
    let mut items = Vec::new();
    skip_ws(bytes, pos);
    if bytes.get(*pos) == Some(&b']') {
        *pos += 1;
        return Ok(Value::Array(items));
    }
    loop {
        let value = parse_value(bytes, pos)?;
        items.push(value);
        skip_ws(bytes, pos);
        match bytes.get(*pos) {
            Some(b',') => {
                *pos += 1;
            }
            Some(b']') => {
                *pos += 1;
                break;
            }
            _ => return Err("expected `,` or `]` in array".into()),
        }
    }
    Ok(Value::Array(items))
}

fn parse_string(bytes: &[u8], pos: &mut usize) -> Result<String, String> {
    if bytes.get(*pos) != Some(&b'"') {
        return Err("expected string".into());
    }
    *pos += 1;
    let mut s = String::new();
    loop {
        match bytes.get(*pos) {
            Some(b'"') => {
                *pos += 1;
                break;
            }
            Some(b'\\') => {
                *pos += 1;
                match bytes.get(*pos) {
                    Some(b'"') => s.push('"'),
                    Some(b'\\') => s.push('\\'),
                    Some(b'/') => s.push('/'),
                    Some(b'n') => s.push('\n'),
                    Some(b't') => s.push('\t'),
                    Some(other) => s.push(*other as char),
                    None => return Err("unterminated escape".into()),
                }
                *pos += 1;
            }
            Some(&c) => {
                s.push(c as char);
                *pos += 1;
            }
            None => return Err("unterminated string".into()),
        }
    }
    Ok(s)
}

fn parse_number(bytes: &[u8], pos: &mut usize) -> Result<Value, String> {
    let start = *pos;
    while bytes
        .get(*pos)
        .is_some_and(|b| b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E'))
    {
        *pos += 1;
    }
    std::str::from_utf8(&bytes[start..*pos])
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Value::Number)
        .ok_or_else(|| "invalid number".into())
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_object_array_and_scalars() {
        let v = parse(r#"{"a":1,"b":[{"c":"x"},2],"d":true}"#).unwrap();
        assert_eq!(v.get("a").unwrap().as_u64(), Some(1));
        let arr = v.get("b").unwrap().as_array().unwrap();
        assert_eq!(arr[0].get("c").unwrap().as_str(), Some("x"));
        assert_eq!(arr[1].as_u64(), Some(2));
    }
}
