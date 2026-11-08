//! Minimal std-only JSON: parse into an order-preserving tree, print compactly.
//!
//! Strings and numbers keep their source text, so printing round-trips exactly
//! and no floating point is involved.

use std::fmt::Write as _;

/// A parsed JSON value.
#[derive(Debug)]
pub enum Value {
    Null,
    Bool(bool),
    /// Number, as written in the source.
    Num(String),
    /// String contents between the quotes, escapes left as written.
    Str(String),
    Arr(Vec<Self>),
    Obj(Vec<(String, Self)>),
}

impl Value {
    /// The value under `key`, if this is an object that has it.
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Arr(items) => Some(items),
            _ => None,
        }
    }

    /// Compact JSON text (no whitespace).
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write_compact(&mut out);
        out
    }

    fn write_compact(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Self::Num(n) => out.push_str(n),
            Self::Str(s) => {
                let _ = write!(out, "\"{s}\"");
            }
            Self::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_compact(out);
                }
                out.push(']');
            }
            Self::Obj(fields) => {
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "\"{key}\":");
                    value.write_compact(out);
                }
                out.push('}');
            }
        }
    }
}

/// Parse a complete JSON document.
pub fn parse(text: &str) -> Result<Value, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos == parser.bytes.len() {
        Ok(value)
    } else {
        Err(parser.error("trailing characters"))
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        format!("json: {what} at byte {}", self.pos)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", char::from(byte))))
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(self.error("bad literal"))
        }
    }

    fn value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(Value::Str),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => Ok(self.number()),
            _ => Err(self.error("unexpected character")),
        }
    }

    fn number(&mut self) -> Value {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.pos += 1;
        }
        Value::Num(String::from_utf8_lossy(&self.bytes[start..self.pos]).into_owned())
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let start = self.pos;
        loop {
            match self.peek() {
                Some(b'"') => break,
                Some(b'\\') => self.pos += 2,
                Some(_) => self.pos += 1,
                None => return Err(self.error("unterminated string")),
            }
        }
        let text = String::from_utf8_lossy(&self.bytes[start..self.pos]).into_owned();
        self.pos += 1;
        Ok(text)
    }

    fn array(&mut self) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Arr(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn object(&mut self) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Obj(fields));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            fields.push((key, self.value()?));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Obj(fields));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }
}
