use crate::error::Error;
use crate::Value;

pub fn parse(src: &str) -> Result<Value, Error> {
    let mut p = Parser { src, pos: 0 };
    p.wsc();
    let pairs = p.pairs(None)?;
    p.wsc();
    if !p.at_end() {
        return Err(p.err(&format!("unexpected '{}'", p.next_char().unwrap())));
    }
    Ok(Value::Map(pairs))
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn at_end(&self) -> bool {
        self.pos >= self.src.len()
    }

    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn next_char(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn err(&self, msg: &str) -> Error {
        Error::at(msg, self.pos)
    }

    // HSPACE = /[ \t]+/
    fn hspace(&mut self) -> bool {
        let start = self.pos;
        while let Some(b' ' | b'\t') = self.peek() {
            self.pos += 1;
        }
        self.pos > start
    }

    // PAIRSEP = /(\r?\n)+|;/
    fn pairsep(&mut self) -> bool {
        if self.peek() == Some(b';') {
            self.pos += 1;
            return true;
        }
        let start = self.pos;
        loop {
            match self.peek() {
                Some(b'\n') => self.pos += 1,
                Some(b'\r') if self.src.as_bytes().get(self.pos + 1) == Some(&b'\n') => {
                    self.pos += 2;
                }
                _ => break,
            }
        }
        self.pos > start
    }

    // COMMENT = '#' /[^\n]*/
    fn comment(&mut self) -> bool {
        if self.peek() == Some(b'#') {
            while let Some(b) = self.peek() {
                if b == b'\n' {
                    break;
                }
                self.pos += 1;
            }
            true
        } else {
            false
        }
    }

    // _ = { WS | COMMENT }
    fn wsc(&mut self) {
        loop {
            if self.peek() == Some(b'#') {
                self.comment();
                continue;
            }
            if let Some(c) = self.next_char() {
                if c.is_whitespace() {
                    self.pos += c.len_utf8();
                    continue;
                }
            }
            break;
        }
    }

    // QSTRING = '"' { QCHAR } '"'
    fn qstring(&mut self) -> Result<String, Error> {
        let open = self.pos;
        self.pos += 1; // skip "

        let mut result = String::new();
        loop {
            if self.at_end() {
                return Err(Error::at("unterminated string", open));
            }
            match self.src.as_bytes()[self.pos] {
                b'"' => {
                    self.pos += 1;
                    return Ok(result);
                }
                b'\\' => {
                    let esc_start = self.pos;
                    self.pos += 1;
                    if self.at_end() {
                        return Err(Error::at("unterminated escape", esc_start));
                    }
                    match self.src.as_bytes()[self.pos] {
                        b'n' => {
                            result.push('\n');
                            self.pos += 1;
                        }
                        b'r' => {
                            result.push('\r');
                            self.pos += 1;
                        }
                        b't' => {
                            result.push('\t');
                            self.pos += 1;
                        }
                        b'\\' => {
                            result.push('\\');
                            self.pos += 1;
                        }
                        b'"' => {
                            result.push('"');
                            self.pos += 1;
                        }
                        b'0' => {
                            result.push('\0');
                            self.pos += 1;
                        }
                        b'\n' => {
                            // line continuation
                            self.pos += 1;
                        }
                        b'u' => {
                            self.pos += 1;
                            if self.at_end() || self.src.as_bytes()[self.pos] != b'{' {
                                return Err(Error::at("expected '{' after \\u", esc_start));
                            }
                            self.pos += 1; // skip {

                            let hex_start = self.pos;
                            while !self.at_end() && self.src.as_bytes()[self.pos] != b'}' {
                                if !self.src.as_bytes()[self.pos].is_ascii_hexdigit() {
                                    return Err(Error::at("invalid unicode escape", esc_start));
                                }
                                self.pos += 1;
                            }

                            let hex = &self.src[hex_start..self.pos];
                            if hex.is_empty()
                                || hex.len() > 6
                                || self.at_end()
                                || self.src.as_bytes()[self.pos] != b'}'
                            {
                                return Err(Error::at("invalid unicode escape", esc_start));
                            }
                            self.pos += 1; // skip }

                            let cp = u32::from_str_radix(hex, 16)
                                .map_err(|_| Error::at("invalid unicode escape", esc_start))?;
                            let c = char::from_u32(cp).ok_or_else(|| {
                                Error::at("unicode codepoint out of range", esc_start)
                            })?;
                            result.push(c);
                        }
                        _ => {
                            let c = self.next_char().unwrap();
                            self.pos += c.len_utf8();
                            return Err(Error::at(&format!("invalid escape '\\{}'", c), esc_start));
                        }
                    }
                }
                _ => {
                    let c = self.next_char().unwrap();
                    result.push(c);
                    self.pos += c.len_utf8();
                }
            }
        }
    }

    // QRSTRING = "'" /[^']*/ "'"
    fn qrstring(&mut self) -> Result<String, Error> {
        let open = self.pos;
        self.pos += 1; // skip '
        let start = self.pos;
        while !self.at_end() && self.src.as_bytes()[self.pos] != b'\'' {
            self.pos += 1;
        }
        let content = self.src[start..self.pos].to_string();
        if !self.at_end() && self.src.as_bytes()[self.pos] == b'\'' {
            self.pos += 1;
            Ok(content)
        } else {
            Err(Error::at("unterminated raw string", open))
        }
    }

    // BARE = /[^\s{}\[\]"#';]+/
    fn bare(&mut self) -> Option<String> {
        let start = self.pos;
        while let Some(c) = self.next_char() {
            if c.is_whitespace() || matches!(c, '{' | '}' | '[' | ']' | '"' | '#' | '\'' | ';') {
                break;
            }
            self.pos += c.len_utf8();
        }
        if self.pos > start {
            Some(self.src[start..self.pos].to_string())
        } else {
            None
        }
    }

    // string = QSTRING | QRSTRING | BARE
    fn string(&mut self) -> Result<Option<String>, Error> {
        match self.peek() {
            Some(b'"') => self.qstring().map(Some),
            Some(b'\'') => self.qrstring().map(Some),
            _ => Ok(self.bare()),
        }
    }

    // value = map | list | string
    fn value(&mut self) -> Result<Option<Value>, Error> {
        match self.peek() {
            Some(b'{') => self.map().map(Some),
            Some(b'[') => self.list().map(Some),
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

        self.hspace();

        let val = match self.value()? {
            Some(v) => v,
            None => {
                return Err(Error::at(
                    &format!("expected value for key '{}'", key),
                    self.pos,
                ))
            }
        };

        self.hspace();
        self.comment();

        Ok(Some((key, val)))
    }

    // pairs = pair { PAIRSEP _ pair }
    fn pairs(&mut self, closing: Option<u8>) -> Result<Vec<(String, Value)>, Error> {
        let mut pairs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        loop {
            if self.at_end() || self.peek() == closing {
                break;
            }

            match self.pair(&mut seen)? {
                Some((k, v)) => pairs.push((k, v)),
                None => {
                    if !self.at_end() && self.peek() != closing {
                        return Err(
                            self.err(&format!("unexpected '{}'", self.next_char().unwrap()))
                        );
                    }
                    break;
                }
            }

            if self.at_end() || self.peek() == closing {
                break;
            }

            if !self.pairsep() {
                return Err(self.err("expected newline or ';' after value"));
            }
            self.wsc();
        }

        Ok(pairs)
    }

    // map = '{' _ [ pairs ] _ '}'
    fn map(&mut self) -> Result<Value, Error> {
        let open = self.pos;
        self.pos += 1; // skip {
        self.wsc();
        let pairs = self.pairs(Some(b'}'))?;
        self.wsc();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            Ok(Value::Map(pairs))
        } else {
            Err(Error::at("unclosed '{'", open))
        }
    }

    // items = value { _ [ ';' ] _ value }
    fn items(&mut self) -> Result<Vec<Value>, Error> {
        let mut items = Vec::new();

        loop {
            if self.at_end() || self.peek() == Some(b']') {
                break;
            }

            match self.value()? {
                Some(v) => items.push(v),
                None => {
                    if !self.at_end() && self.peek() != Some(b']') {
                        return Err(
                            self.err(&format!("unexpected '{}'", self.next_char().unwrap()))
                        );
                    }
                    break;
                }
            }

            if self.at_end() || self.peek() == Some(b']') {
                break;
            }

            self.wsc();
            if self.peek() == Some(b';') {
                self.pos += 1;
            }
            self.wsc();
        }

        Ok(items)
    }

    // list = '[' _ [ items ] _ ']'
    fn list(&mut self) -> Result<Value, Error> {
        let open = self.pos;
        self.pos += 1; // skip [
        self.wsc();
        let items = self.items()?;
        self.wsc();
        if self.peek() == Some(b']') {
            self.pos += 1;
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
        let v = parse("name foo").unwrap();
        assert_eq!(v, Value::Map(vec![("name".into(), s("foo"))]));
    }

    #[test]
    fn multiple_pairs() {
        let v = parse("a 1\nb 2").unwrap();
        assert_eq!(
            v,
            Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
        );
    }

    #[test]
    fn semicolon_sep() {
        let v = parse("a 1; b 2").unwrap();
        assert_eq!(
            v,
            Value::Map(vec![("a".into(), s("1")), ("b".into(), s("2"))])
        );
    }

    #[test]
    fn nested_map() {
        let v = parse("x { a 1; b 2 }").unwrap();
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
        let v = parse("tags [a b c]").unwrap();
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
        let v = parse(r#"msg "hello\nworld""#).unwrap();
        assert_eq!(v, Value::Map(vec![("msg".into(), s("hello\nworld"))]));
    }

    #[test]
    fn raw_string() {
        let v = parse(r"path 'C:\foo\bar'").unwrap();
        assert_eq!(v, Value::Map(vec![("path".into(), s(r"C:\foo\bar"))]));
    }

    #[test]
    fn unicode_escape() {
        let v = parse(r#"ch "\u{1f331}""#).unwrap();
        assert_eq!(v, Value::Map(vec![("ch".into(), s("\u{1f331}"))]))
    }

    #[test]
    fn comment() {
        let v = parse("# header\na 1 # inline\n").unwrap();
        assert_eq!(v, Value::Map(vec![("a".into(), s("1"))]));
    }

    #[test]
    fn line_continuation() {
        let v = parse("msg \"hello \\\nworld\"").unwrap();
        assert_eq!(v, Value::Map(vec![("msg".into(), s("hello world"))]));
    }

    #[test]
    fn empty() {
        let v = parse("").unwrap();
        assert_eq!(v, Value::Map(vec![]));
    }

    #[test]
    fn whitespace_only() {
        let v = parse("  \n\n  ").unwrap();
        assert_eq!(v, Value::Map(vec![]));
    }

    #[test]
    fn unclosed_brace() {
        assert!(parse("x {").is_err());
    }

    #[test]
    fn unclosed_bracket() {
        assert!(parse("x [").is_err());
    }

    #[test]
    fn unterminated_string() {
        assert!(parse(r#"x "hello"#).is_err());
    }

    #[test]
    fn invalid_escape() {
        assert!(parse(r#"x "\q""#).is_err());
    }

    #[test]
    fn duplicate_key_toplevel() {
        let e = parse("a 1\na 2").unwrap_err();
        assert!(e.msg.contains("duplicate key 'a'"), "{}", e.msg);
    }

    #[test]
    fn duplicate_key_nested() {
        let e = parse("x { k 1; k 2 }").unwrap_err();
        assert!(e.msg.contains("duplicate key 'k'"), "{}", e.msg);
    }
}
