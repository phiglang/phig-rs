//! Event-driven parser for the phig format.

use std::io::Read;

use crate::error::Error;

/// Handler for phig parser events.
///
/// Implement this trait to receive events from [`parse`] without constructing
/// an intermediate [`crate::Value`] tree.
pub trait Handler {
    /// The error type returned by handler methods.
    type Error: From<Error>;

    /// Called when a map begins (`{` or the implicit top-level map).
    fn map_start(&mut self) -> Result<(), Self::Error>;
    /// Called when a map ends (`}` or the implicit top-level map).
    fn map_end(&mut self) -> Result<(), Self::Error>;
    /// Called when a list begins (`[`).
    fn list_start(&mut self) -> Result<(), Self::Error>;
    /// Called when a list ends (`]`).
    fn list_end(&mut self) -> Result<(), Self::Error>;
    /// Called for each key in a map, immediately before its value event.
    fn key(&mut self, key: String) -> Result<(), Self::Error>;
    /// Called for each string value.
    fn string(&mut self, value: String) -> Result<(), Self::Error>;
}

/// Parse phig input and send events to a [`Handler`].
pub fn parse<H: Handler>(reader: impl Read, handler: &mut H) -> Result<(), H::Error> {
    let mut p = Parser::new(reader);
    p.skip_bom()?;
    p.wsc()?;
    handler.map_start()?;
    p.pairs(None, handler)?;
    handler.map_end()?;
    p.wsc()?;
    if !p.at_end()? {
        let c = p.peek()?.unwrap();
        return Err(p.err(&format!("unexpected '{}'", c)).into());
    }
    Ok(())
}

struct Parser<R: Read> {
    reader: R,
    buf: Vec<u8>,
    pos: usize,
}

impl<R: Read> Parser<R> {
    fn new(reader: R) -> Self {
        Parser {
            reader,
            buf: Vec::new(),
            pos: 0,
        }
    }

    /// Ensure at least `n` bytes in the lookahead buffer.
    fn fill(&mut self, n: usize) -> Result<usize, Error> {
        while self.buf.len() < n {
            let mut byte = [0u8; 1];
            match self.reader.read(&mut byte)? {
                0 => break,
                _ => self.buf.push(byte[0]),
            }
        }
        Ok(self.buf.len())
    }

    fn at_end(&mut self) -> Result<bool, Error> {
        Ok(self.fill(1)? == 0)
    }

    fn peek(&mut self) -> Result<Option<char>, Error> {
        if self.fill(1)? == 0 {
            return Ok(None);
        }
        let first = self.buf[0];
        let len = match first {
            0x00..0x80 => 1,
            0xC0..0xE0 => 2,
            0xE0..0xF0 => 3,
            0xF0..0xF8 => 4,
            _ => return Err(Error::at("invalid UTF-8", self.pos)),
        };
        if self.fill(len)? < len {
            return Err(Error::at("incomplete UTF-8 character", self.pos));
        }
        match std::str::from_utf8(&self.buf[..len]) {
            Ok(s) => Ok(s.chars().next()),
            Err(_) => Err(Error::at("invalid UTF-8", self.pos)),
        }
    }

    fn advance(&mut self) -> Result<Option<char>, Error> {
        let c = match self.peek()? {
            Some(c) => c,
            None => return Ok(None),
        };
        for _ in 0..c.len_utf8() {
            self.buf.remove(0);
            self.pos += 1;
        }
        Ok(Some(c))
    }

    fn err(&self, msg: &str) -> Error {
        Error::at(msg, self.pos)
    }

    /// Skip an optional UTF-8 BOM (EF BB BF) at the start of input.
    fn skip_bom(&mut self) -> Result<(), Error> {
        if self.fill(3)? >= 3 && self.buf[..3] == [0xEF, 0xBB, 0xBF] {
            self.buf.drain(..3);
            self.pos += 3;
        }
        Ok(())
    }

    // HSPACE = /[ \t]+/
    fn hspace(&mut self) -> Result<bool, Error> {
        let start = self.pos;
        while matches!(self.peek()?, Some(' ' | '\t')) {
            self.advance()?;
        }
        Ok(self.pos > start)
    }

    // PAIRSEP = /(\r?\n)+|;/
    fn pairsep(&mut self) -> Result<bool, Error> {
        if self.peek()? == Some(';') {
            self.advance()?;
            return Ok(true);
        }
        let start = self.pos;
        loop {
            match self.peek()? {
                Some('\n') => {
                    self.advance()?;
                }
                Some('\r') => {
                    self.advance()?;
                    if self.peek()? == Some('\n') {
                        self.advance()?;
                    }
                }
                _ => break,
            }
        }
        Ok(self.pos > start)
    }

    // COMMENT = '#' /[^\n]*/
    fn comment(&mut self) -> Result<bool, Error> {
        if self.peek()? == Some('#') {
            loop {
                match self.peek()? {
                    Some('\n') | None => break,
                    _ => {
                        self.advance()?;
                    }
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // _ = { WS | COMMENT }
    fn wsc(&mut self) -> Result<(), Error> {
        loop {
            match self.peek()? {
                Some('#') => {
                    self.comment()?;
                }
                Some(' ' | '\t' | '\n' | '\r') => {
                    self.advance()?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    // QSTRING = '"' { QCHAR } '"'
    fn qstring(&mut self) -> Result<String, Error> {
        let open = self.pos;
        self.advance()?; // skip "

        let mut result = String::new();
        loop {
            let Some(ch) = self.peek()? else {
                return Err(Error::at("unterminated string", open));
            };
            match ch {
                '"' => {
                    self.advance()?;
                    return Ok(result);
                }
                '\\' => {
                    let esc_start = self.pos;
                    self.advance()?;
                    let Some(esc) = self.peek()? else {
                        return Err(Error::at("unterminated escape", esc_start));
                    };
                    match esc {
                        'n' => {
                            result.push('\n');
                            self.advance()?;
                        }
                        'r' => {
                            result.push('\r');
                            self.advance()?;
                        }
                        't' => {
                            result.push('\t');
                            self.advance()?;
                        }
                        '\\' => {
                            result.push('\\');
                            self.advance()?;
                        }
                        '"' => {
                            result.push('"');
                            self.advance()?;
                        }
                        '0' => {
                            result.push('\0');
                            self.advance()?;
                        }
                        '\r' => {
                            // line continuation: \<CR><LF>
                            self.advance()?;
                            if self.peek()? == Some('\n') {
                                self.advance()?;
                            } else {
                                return Err(Error::at(
                                    "expected LF after CR in line continuation",
                                    esc_start,
                                ));
                            }
                        }
                        '\n' => {
                            // line continuation: \<LF>
                            self.advance()?;
                        }
                        'u' => {
                            self.advance()?;
                            if self.peek()? != Some('{') {
                                return Err(Error::at("expected '{' after \\u", esc_start));
                            }
                            self.advance()?; // skip {

                            let mut hex = String::new();
                            loop {
                                let Some(hc) = self.peek()? else {
                                    return Err(Error::at("invalid unicode escape", esc_start));
                                };
                                if hc == '}' {
                                    break;
                                }
                                if !hc.is_ascii_hexdigit() {
                                    return Err(Error::at("invalid unicode escape", esc_start));
                                }
                                hex.push(hc);
                                self.advance()?;
                            }

                            if hex.is_empty() || hex.len() > 6 {
                                return Err(Error::at("invalid unicode escape", esc_start));
                            }
                            self.advance()?; // skip }

                            let cp = u32::from_str_radix(&hex, 16)
                                .map_err(|_| Error::at("invalid unicode escape", esc_start))?;
                            let c = char::from_u32(cp).ok_or_else(|| {
                                Error::at("unicode codepoint out of range", esc_start)
                            })?;
                            result.push(c);
                        }
                        _ => {
                            self.advance()?;
                            return Err(Error::at(
                                &format!("invalid escape '\\{}'", esc),
                                esc_start,
                            ));
                        }
                    }
                }
                _ => {
                    result.push(self.advance()?.unwrap());
                }
            }
        }
    }

    // QRSTRING = "'" /[^']*/ "'"
    fn qrstring(&mut self) -> Result<String, Error> {
        let open = self.pos;
        self.advance()?; // skip '
        let mut result = String::new();
        loop {
            let Some(ch) = self.peek()? else {
                return Err(Error::at("unterminated raw string", open));
            };
            if ch == '\'' {
                self.advance()?;
                return Ok(result);
            }
            result.push(self.advance()?.unwrap());
        }
    }

    // BARE = /[^\p{White_Space}{}[\]"#';]+/
    fn bare(&mut self) -> Result<Option<String>, Error> {
        let mut result = String::new();
        loop {
            let Some(c) = self.peek()? else { break };
            if c.is_whitespace() || matches!(c, '{' | '}' | '[' | ']' | '"' | '#' | '\'' | ';') {
                break;
            }
            self.advance()?;
            result.push(c);
        }
        if result.is_empty() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    // string = QSTRING | QRSTRING | BARE
    fn string(&mut self) -> Result<Option<String>, Error> {
        match self.peek()? {
            Some('"') => self.qstring().map(Some),
            Some('\'') => self.qrstring().map(Some),
            _ => self.bare(),
        }
    }

    // value = map | list | string
    fn value<H: Handler>(&mut self, handler: &mut H) -> Result<bool, H::Error> {
        match self.peek()? {
            Some('{') => {
                self.map(handler)?;
                Ok(true)
            }
            Some('[') => {
                self.list(handler)?;
                Ok(true)
            }
            _ => match self.string()? {
                Some(s) => {
                    handler.string(s)?;
                    Ok(true)
                }
                None => Ok(false),
            },
        }
    }

    // pair = string HSPACE value [ HSPACE ] [ COMMENT ]
    fn pair<H: Handler>(
        &mut self,
        seen: &mut std::collections::HashSet<String>,
        handler: &mut H,
    ) -> Result<bool, H::Error> {
        let start = self.pos;
        let key = match self.string()? {
            Some(k) => k,
            None => return Ok(false),
        };

        if !seen.insert(key.clone()) {
            return Err(Error::at(&format!("duplicate key '{}'", key), start).into());
        }

        self.hspace()?;
        handler.key(key)?;

        if !self.value(handler)? {
            return Err(self.err("expected value after key").into());
        }

        self.hspace()?;
        self.comment()?;

        Ok(true)
    }

    // pairs = pair { PAIRSEP _ pair }
    fn pairs<H: Handler>(
        &mut self,
        closing: Option<char>,
        handler: &mut H,
    ) -> Result<(), H::Error> {
        let mut seen = std::collections::HashSet::new();

        loop {
            if self.at_end()? || self.peek()? == closing {
                break;
            }

            if !self.pair(&mut seen, handler)? {
                if !self.at_end()? && self.peek()? != closing {
                    let c = self.peek()?.unwrap();
                    return Err(self.err(&format!("unexpected '{}'", c)).into());
                }
                break;
            }

            if self.at_end()? || self.peek()? == closing {
                break;
            }

            if !self.pairsep()? {
                return Err(self.err("expected newline or ';' after value").into());
            }
            self.wsc()?;
        }

        Ok(())
    }

    // map = '{' _ [ pairs ] _ '}'
    fn map<H: Handler>(&mut self, handler: &mut H) -> Result<(), H::Error> {
        let open = self.pos;
        self.advance()?; // skip {
        self.wsc()?;
        handler.map_start()?;
        self.pairs(Some('}'), handler)?;
        self.wsc()?;
        if self.peek()? == Some('}') {
            self.advance()?;
            handler.map_end()?;
            Ok(())
        } else {
            Err(Error::at("unclosed '{'", open).into())
        }
    }

    // items = value { _ [ ';' ] _ value }
    fn items<H: Handler>(&mut self, handler: &mut H) -> Result<(), H::Error> {
        loop {
            if self.at_end()? || self.peek()? == Some(']') {
                break;
            }

            if !self.value(handler)? {
                if !self.at_end()? && self.peek()? != Some(']') {
                    let c = self.peek()?.unwrap();
                    return Err(self.err(&format!("unexpected '{}'", c)).into());
                }
                break;
            }

            if self.at_end()? || self.peek()? == Some(']') {
                break;
            }

            self.wsc()?;
            if self.peek()? == Some(';') {
                self.advance()?;
            }
            self.wsc()?;
        }

        Ok(())
    }

    // list = '[' _ [ items ] _ ']'
    fn list<H: Handler>(&mut self, handler: &mut H) -> Result<(), H::Error> {
        let open = self.pos;
        self.advance()?; // skip [
        self.wsc()?;
        handler.list_start()?;
        self.items(handler)?;
        self.wsc()?;
        if self.peek()? == Some(']') {
            self.advance()?;
            handler.list_end()?;
            Ok(())
        } else {
            Err(Error::at("unclosed '['", open).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    fn p(input: &[u8]) -> Result<Value, Error> {
        let mut builder = crate::ValueBuilder::new();
        parse(input, &mut builder)?;
        Ok(builder.finish())
    }

    fn s(v: &str) -> Value {
        Value::String(v.to_string())
    }

    #[test]
    fn bare_pairs() {
        let v = p("name foo".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("name".into(), s("foo"))]));
    }

    #[test]
    fn multiple_pairs() {
        let v = p("a 1\nb 2".as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
        );
    }

    #[test]
    fn semicolon_sep() {
        let v = p("a 1; b 2".as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
        );
    }

    #[test]
    fn nested_map() {
        let v = p("x { a 1; b 2 }".as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Map(vec![(
                "x".into(),
                Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
            )])
        );
    }

    #[test]
    fn list() {
        let v = p("tags [a b c]".as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Map(vec![(
                "tags".into(),
                Value::List(vec![s("a"), s("b"), s("c")])
            )])
        );
    }

    #[test]
    fn quoted_string() {
        let v = p(r#"msg "hello\nworld""#.as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("msg".into(), s("hello\nworld"))]));
    }

    #[test]
    fn raw_string() {
        let v = p(r"path 'C:\foo\bar'".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("path".into(), s(r"C:\foo\bar"))]));
    }

    #[test]
    fn unicode_escape() {
        let v = p(r#"ch "\u{1f331}""#.as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("ch".into(), s("\u{1f331}"))]))
    }

    #[test]
    fn comment() {
        let v = p("# header\na 1 # inline\n".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("a".into(), s("1"))]));
    }

    #[test]
    fn line_continuation() {
        let v = p("msg \"hello \\\nworld\"".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("msg".into(), s("hello world"))]));
    }

    #[test]
    fn empty() {
        let v = p("".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![]));
    }

    #[test]
    fn whitespace_only() {
        let v = p("  \n\n  ".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![]));
    }

    #[test]
    fn unclosed_brace() {
        assert!(p("x {".as_bytes()).is_err());
    }

    #[test]
    fn unclosed_bracket() {
        assert!(p("x [".as_bytes()).is_err());
    }

    #[test]
    fn unterminated_string() {
        assert!(p(r#"x "hello"#.as_bytes()).is_err());
    }

    #[test]
    fn invalid_escape() {
        assert!(p(r#"x "\q""#.as_bytes()).is_err());
    }

    #[test]
    fn duplicate_key_toplevel() {
        let e = p("a 1\na 2".as_bytes()).unwrap_err();
        assert!(e.msg().unwrap().contains("duplicate key 'a'"), "{}", e);
    }

    #[test]
    fn duplicate_key_nested() {
        let e = p("x { k 1; k 2 }".as_bytes()).unwrap_err();
        assert!(e.msg().unwrap().contains("duplicate key 'k'"), "{}", e);
    }

    #[test]
    fn nbsp_not_separator() {
        assert!(p("name\u{00a0}foo".as_bytes()).is_err());
    }

    #[test]
    fn nbsp_in_bare_value() {
        assert!(p("name foo\u{00a0}bar".as_bytes()).is_err());
    }

    #[test]
    fn em_space_in_bare_value() {
        assert!(p("name foo\u{2003}bar".as_bytes()).is_err());
    }

    #[test]
    fn nbsp_in_quoted_ok() {
        let v = p("name \"foo\u{00a0}bar\"".as_bytes()).unwrap();
        assert_eq!(v["name"].as_str(), Some("foo\u{00a0}bar"));
    }

    #[test]
    fn nbsp_in_raw_ok() {
        let v = p("name 'foo\u{00a0}bar'".as_bytes()).unwrap();
        assert_eq!(v["name"].as_str(), Some("foo\u{00a0}bar"));
    }
}
