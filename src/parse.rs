use std::io::Read;

use crate::error::Error;
use crate::Value;

pub fn parse(reader: impl Read) -> Result<Value, Error> {
    let mut p = Parser::new(reader);
    p.wsc()?;
    let pairs = p.pairs(None)?;
    p.wsc()?;
    if !p.at_end()? {
        let c = p.peek()?.unwrap();
        return Err(p.err(&format!("unexpected '{}'", c)));
    }
    Ok(Value::Map(pairs))
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
    fn value(&mut self) -> Result<Option<Value>, Error> {
        match self.peek()? {
            Some('{') => self.map().map(Some),
            Some('[') => self.list().map(Some),
            _ => match self.string()? {
                Some(s) => Ok(Some(Value::String(s))),
                None => Ok(None),
            },
        }
    }

    // pair = string HSPACE value [ HSPACE ] [ COMMENT ]
    fn pair(
        &mut self,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<Option<(String, Value)>, Error> {
        let start = self.pos;
        let key = match self.string()? {
            Some(k) => k,
            None => return Ok(None),
        };

        if !seen.insert(key.clone()) {
            return Err(Error::at(&format!("duplicate key '{}'", key), start));
        }

        self.hspace()?;

        let val = match self.value()? {
            Some(v) => v,
            None => {
                return Err(Error::at(
                    &format!("expected value for key '{}'", key),
                    self.pos,
                ))
            }
        };

        self.hspace()?;
        self.comment()?;

        Ok(Some((key, val)))
    }

    // pairs = pair { PAIRSEP _ pair }
    fn pairs(&mut self, closing: Option<char>) -> Result<Vec<(String, Value)>, Error> {
        let mut pairs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        loop {
            if self.at_end()? || self.peek()? == closing {
                break;
            }

            match self.pair(&mut seen)? {
                Some((k, v)) => pairs.push((k, v)),
                None => {
                    if !self.at_end()? && self.peek()? != closing {
                        let c = self.peek()?.unwrap();
                        return Err(self.err(&format!("unexpected '{}'", c)));
                    }
                    break;
                }
            }

            if self.at_end()? || self.peek()? == closing {
                break;
            }

            if !self.pairsep()? {
                return Err(self.err("expected newline or ';' after value"));
            }
            self.wsc()?;
        }

        Ok(pairs)
    }

    // map = '{' _ [ pairs ] _ '}'
    fn map(&mut self) -> Result<Value, Error> {
        let open = self.pos;
        self.advance()?; // skip {
        self.wsc()?;
        let pairs = self.pairs(Some('}'))?;
        self.wsc()?;
        if self.peek()? == Some('}') {
            self.advance()?;
            Ok(Value::Map(pairs))
        } else {
            Err(Error::at("unclosed '{'", open))
        }
    }

    // items = value { _ [ ';' ] _ value }
    fn items(&mut self) -> Result<Vec<Value>, Error> {
        let mut items = Vec::new();

        loop {
            if self.at_end()? || self.peek()? == Some(']') {
                break;
            }

            match self.value()? {
                Some(v) => items.push(v),
                None => {
                    if !self.at_end()? && self.peek()? != Some(']') {
                        let c = self.peek()?.unwrap();
                        return Err(self.err(&format!("unexpected '{}'", c)));
                    }
                    break;
                }
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

        Ok(items)
    }

    // list = '[' _ [ items ] _ ']'
    fn list(&mut self) -> Result<Value, Error> {
        let open = self.pos;
        self.advance()?; // skip [
        self.wsc()?;
        let items = self.items()?;
        self.wsc()?;
        if self.peek()? == Some(']') {
            self.advance()?;
            Ok(Value::List(items))
        } else {
            Err(Error::at("unclosed '['", open))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value {
        Value::String(v.to_string())
    }

    #[test]
    fn bare_pairs() {
        let v = parse("name foo".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("name".into(), s("foo"))]));
    }

    #[test]
    fn multiple_pairs() {
        let v = parse("a 1\nb 2".as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
        );
    }

    #[test]
    fn semicolon_sep() {
        let v = parse("a 1; b 2".as_bytes()).unwrap();
        assert_eq!(
            v,
            Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
        );
    }

    #[test]
    fn nested_map() {
        let v = parse("x { a 1; b 2 }".as_bytes()).unwrap();
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
        let v = parse("tags [a b c]".as_bytes()).unwrap();
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
        let v = parse(r#"msg "hello\nworld""#.as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("msg".into(), s("hello\nworld"))]));
    }

    #[test]
    fn raw_string() {
        let v = parse(r"path 'C:\foo\bar'".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("path".into(), s(r"C:\foo\bar"))]));
    }

    #[test]
    fn unicode_escape() {
        let v = parse(r#"ch "\u{1f331}""#.as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("ch".into(), s("\u{1f331}"))]))
    }

    #[test]
    fn comment() {
        let v = parse("# header\na 1 # inline\n".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("a".into(), s("1"))]));
    }

    #[test]
    fn line_continuation() {
        let v = parse("msg \"hello \\\nworld\"".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![("msg".into(), s("hello world"))]));
    }

    #[test]
    fn empty() {
        let v = parse("".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![]));
    }

    #[test]
    fn whitespace_only() {
        let v = parse("  \n\n  ".as_bytes()).unwrap();
        assert_eq!(v, Value::Map(vec![]));
    }

    #[test]
    fn unclosed_brace() {
        assert!(parse("x {".as_bytes()).is_err());
    }

    #[test]
    fn unclosed_bracket() {
        assert!(parse("x [".as_bytes()).is_err());
    }

    #[test]
    fn unterminated_string() {
        assert!(parse(r#"x "hello"#.as_bytes()).is_err());
    }

    #[test]
    fn invalid_escape() {
        assert!(parse(r#"x "\q""#.as_bytes()).is_err());
    }

    #[test]
    fn duplicate_key_toplevel() {
        let e = parse("a 1\na 2".as_bytes()).unwrap_err();
        assert!(e.msg.contains("duplicate key 'a'"), "{}", e.msg);
    }

    #[test]
    fn duplicate_key_nested() {
        let e = parse("x { k 1; k 2 }".as_bytes()).unwrap_err();
        assert!(e.msg.contains("duplicate key 'k'"), "{}", e.msg);
    }

    #[test]
    fn nbsp_not_separator() {
        assert!(parse("name\u{00a0}foo".as_bytes()).is_err());
    }

    #[test]
    fn nbsp_in_bare_value() {
        assert!(parse("name foo\u{00a0}bar".as_bytes()).is_err());
    }

    #[test]
    fn em_space_in_bare_value() {
        assert!(parse("name foo\u{2003}bar".as_bytes()).is_err());
    }

    #[test]
    fn nbsp_in_quoted_ok() {
        let v = parse("name \"foo\u{00a0}bar\"".as_bytes()).unwrap();
        assert_eq!(v["name"].as_str(), Some("foo\u{00a0}bar"));
    }

    #[test]
    fn nbsp_in_raw_ok() {
        let v = parse("name 'foo\u{00a0}bar'".as_bytes()).unwrap();
        assert_eq!(v["name"].as_str(), Some("foo\u{00a0}bar"));
    }
}
