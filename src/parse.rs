//! Pull-based parser for the phig format.

use std::collections::HashSet;
use std::io::Read;

use crate::error::Error;

/// Events emitted by [`PhigParser`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    StartMap,
    EndMap,
    Key(String),
    StartList,
    EndList,
    String(String),
}

/// Pull-based parser for the phig configuration language.
///
/// Each call to [`next`](PhigParser::next) returns the next parse [`Event`],
/// or `None` when the input is fully consumed.
///
/// Parser states correspond to positions in the grammar:
/// ```text
///   toplevel = [BOM] _ [pairs] _ EOF
///   value    = map | list | string
///   map      = '{' _ [pairs] _ '}'
///   pairs    = pair { PAIRSEP _ pair }
///   pair     = string [HSPACE] value [HSPACE] [COMMENT]
///   list     = '[' _ [items] _ ']'
///   items    = value { _ [';'] _ value }
/// ```
pub struct PhigParser<R: Read> {
    reader: R,
    buf: Vec<u8>,
    pos: usize,
    state: State,
    stack: Vec<Frame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// toplevel: before BOM / first pair.
    DocStart,
    /// pairs: expecting a key or end of container.
    BeforePair,
    /// pair: key parsed, expecting value.
    AfterKey,
    /// pairs: pair done, expecting PAIRSEP or end of container.
    BetweenPairs,
    /// items: expecting a value or ']'.
    BeforeItem,
    /// items: item done, expecting separator or ']'.
    BetweenItems,
    /// Document fully consumed.
    Done,
}

struct Frame {
    is_list: bool,
    closing: Option<char>,
    open_pos: usize,
    seen: Option<HashSet<String>>,
}

impl<R: Read> PhigParser<R> {
    /// Create a new parser that reads from `reader`.
    pub fn new(reader: R) -> Self {
        PhigParser {
            reader,
            buf: Vec::new(),
            pos: 0,
            state: State::DocStart,
            stack: Vec::new(),
        }
    }

    fn next_event(&mut self) -> Result<Option<Event>, Error> {
        loop {
            match self.state {
                // toplevel = [BOM] _ [pairs] _ EOF
                State::DocStart => {
                    self.skip_bom()?;
                    self.wsc()?;
                    self.stack.push(Frame {
                        is_list: false,
                        closing: None,
                        open_pos: 0,
                        seen: Some(HashSet::new()),
                    });
                    self.state = State::BeforePair;
                    return Ok(Some(Event::StartMap));
                }

                // pairs = pair { PAIRSEP _ pair }
                //         ^--- expecting a key (start of pair) or end of map
                State::BeforePair => {
                    if self.at_map_end()? {
                        return self.close_map().map(Some);
                    }
                    return self.parse_key().map(Some);
                }

                // pair = string [HSPACE] value [HSPACE] [COMMENT]
                //                        ^--- key parsed, expecting value
                State::AfterKey => {
                    return self.parse_value(State::BetweenPairs).map(Some);
                }

                // pairs = pair { PAIRSEP _ pair }
                //              ^--- pair done, expecting PAIRSEP or end of map
                State::BetweenPairs => {
                    self.hspace()?; // [HSPACE] trailing the pair
                    self.comment()?; // [COMMENT] trailing the pair
                    if self.at_map_end()? {
                        self.state = State::BeforePair;
                        continue;
                    }
                    if !self.pairsep()? {
                        return Err(self.err("expected newline or ';' after value"));
                    }
                    self.wsc()?; // _ before next pair
                    self.state = State::BeforePair;
                }

                // items = value { _ [';'] _ value }
                //         ^--- expecting a value or ']'
                State::BeforeItem => {
                    if self.at_list_end()? {
                        return self.close_list().map(Some);
                    }
                    return self.parse_value(State::BetweenItems).map(Some);
                }

                // items = value { _ [';'] _ value }
                //              ^--- item done, expecting separator or ']'
                State::BetweenItems => {
                    if self.at_end()? || self.peek()? == Some(']') {
                        self.state = State::BeforeItem;
                        continue;
                    }
                    self.wsc()?;
                    if self.peek()? == Some(';') {
                        self.advance()?;
                    }
                    self.wsc()?;
                    self.state = State::BeforeItem;
                }

                State::Done => {
                    return Ok(None);
                }
            }
        }
    }

    // ── grammar-level helpers ──────────────────────────────────────

    /// Parses a key at the start of a pair, checks for duplicates,
    /// consumes trailing HSPACE, and returns [`Event::Key`].
    fn parse_key(&mut self) -> Result<Event, Error> {
        let start = self.pos;
        let key = match self.string_val()? {
            Some(k) => k,
            None => {
                let c = self.peek()?.unwrap();
                return Err(Error::at(&format!("unexpected '{}'", c), self.pos));
            }
        };

        {
            let frame = self.stack.last_mut().unwrap();
            let seen = frame.seen.as_mut().unwrap();
            if seen.contains(&key) {
                return Err(Error::at(&format!("duplicate key '{}'", key), start));
            }
            seen.insert(key.clone());
        }

        self.hspace()?; // [HSPACE] between key and value
        self.state = State::AfterKey;
        Ok(Event::Key(key))
    }

    /// Parses a value (map, list, or string) and returns the
    /// corresponding event.
    ///
    /// `after_string` is the state to transition to when the value is a
    /// plain string (for maps/lists the state is set to the
    /// container-interior state instead).
    fn parse_value(&mut self, after_string: State) -> Result<Event, Error> {
        match self.peek()? {
            Some('{') => {
                let open = self.pos;
                self.advance()?;
                self.wsc()?;
                self.stack.push(Frame {
                    is_list: false,
                    closing: Some('}'),
                    open_pos: open,
                    seen: Some(HashSet::new()),
                });
                self.state = State::BeforePair;
                Ok(Event::StartMap)
            }
            Some('[') => {
                let open = self.pos;
                self.advance()?;
                self.wsc()?;
                self.stack.push(Frame {
                    is_list: true,
                    closing: Some(']'),
                    open_pos: open,
                    seen: None,
                });
                self.state = State::BeforeItem;
                Ok(Event::StartList)
            }
            cp => {
                let s = self.string_val()?;
                match s {
                    Some(s) => {
                        self.state = after_string;
                        Ok(Event::String(s))
                    }
                    None => {
                        let msg = if after_string == State::BetweenPairs {
                            "expected value after key".to_string()
                        } else {
                            format!("unexpected '{}'", cp.unwrap())
                        };
                        Err(Error::at(&msg, self.pos))
                    }
                }
            }
        }
    }

    /// True when the current position is at the end of the innermost map.
    fn at_map_end(&mut self) -> Result<bool, Error> {
        let closing = self.stack.last().unwrap().closing;
        match closing {
            None => self.at_end(),
            Some(c) => Ok(self.at_end()? || self.peek()? == Some(c)),
        }
    }

    /// True when the current position is at the end of the innermost list.
    fn at_list_end(&mut self) -> Result<bool, Error> {
        Ok(self.at_end()? || self.peek()? == Some(']'))
    }

    /// Closes the current map: consumes '}' (or verifies EOF for
    /// top-level), pops the frame, and returns [`Event::EndMap`].
    fn close_map(&mut self) -> Result<Event, Error> {
        let closing = self.stack.last().unwrap().closing;
        let open_pos = self.stack.last().unwrap().open_pos;

        match closing {
            Some(c) => {
                if self.peek()? != Some(c) {
                    return Err(Error::at("unclosed '{'", open_pos));
                }
                self.advance()?;
            }
            None => {
                self.wsc()?;
                if !self.at_end()? {
                    let c = self.peek()?.unwrap();
                    return Err(Error::at(&format!("unexpected '{}'", c), self.pos));
                }
            }
        }

        self.stack.pop();
        self.after_container();
        Ok(Event::EndMap)
    }

    /// Closes the current list: consumes ']', pops the frame,
    /// and returns [`Event::EndList`].
    fn close_list(&mut self) -> Result<Event, Error> {
        let open_pos = self.stack.last().unwrap().open_pos;
        if self.peek()? != Some(']') {
            return Err(Error::at("unclosed '['", open_pos));
        }
        self.advance()?;
        self.stack.pop();
        self.after_container();
        Ok(Event::EndList)
    }

    /// Sets the state appropriate for the parent context after closing a container.
    fn after_container(&mut self) {
        if self.stack.is_empty() {
            self.state = State::Done;
        } else {
            let parent = self.stack.last().unwrap();
            self.state = if parent.is_list {
                State::BetweenItems
            } else {
                State::BetweenPairs
            };
        }
    }

    // ── low-level scanning ──────────────────────────────────────────

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

    /// Consumes horizontal whitespace (space and tab only).
    fn hspace(&mut self) -> Result<bool, Error> {
        let start = self.pos;
        while matches!(self.peek()?, Some(' ' | '\t')) {
            self.advance()?;
        }
        Ok(self.pos > start)
    }

    /// Consumes a pair separator: one or more newlines, or a semicolon.
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

    /// Consumes a # comment to end of line.
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

    /// Consumes structural whitespace (space, tab, CR, LF) and comments.
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

    /// Parses a double-quoted string with escape sequences.
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

    /// Parses a single-quoted raw string (no escapes).
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

    /// Parses a bare (unquoted) string. Returns `None` if no characters consumed.
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

    /// Parses a string value (quoted, raw, or bare). Returns `None` if none found.
    fn string_val(&mut self) -> Result<Option<String>, Error> {
        match self.peek()? {
            Some('"') => self.qstring().map(Some),
            Some('\'') => self.qrstring().map(Some),
            _ => self.bare(),
        }
    }
}

impl<R: Read> Iterator for PhigParser<R> {
    type Item = Result<Event, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_event().transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    fn p(input: &[u8]) -> Result<Value, Error> {
        crate::value::parse_to_value(input)
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

    #[test]
    fn pull_parser_events() {
        let mut parser = PhigParser::new("a 1\nb 2".as_bytes());
        assert_eq!(parser.next().unwrap().unwrap(), Event::StartMap);
        assert_eq!(parser.next().unwrap().unwrap(), Event::Key("a".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::String("1".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::Key("b".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::String("2".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::EndMap);
        assert!(parser.next().is_none());
    }

    #[test]
    fn pull_parser_nested() {
        let mut parser = PhigParser::new("x { a 1 }".as_bytes());
        assert_eq!(parser.next().unwrap().unwrap(), Event::StartMap);
        assert_eq!(parser.next().unwrap().unwrap(), Event::Key("x".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::StartMap);
        assert_eq!(parser.next().unwrap().unwrap(), Event::Key("a".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::String("1".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::EndMap);
        assert_eq!(parser.next().unwrap().unwrap(), Event::EndMap);
        assert!(parser.next().is_none());
    }

    #[test]
    fn pull_parser_list() {
        let mut parser = PhigParser::new("tags [a b]".as_bytes());
        assert_eq!(parser.next().unwrap().unwrap(), Event::StartMap);
        assert_eq!(parser.next().unwrap().unwrap(), Event::Key("tags".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::StartList);
        assert_eq!(parser.next().unwrap().unwrap(), Event::String("a".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::String("b".into()));
        assert_eq!(parser.next().unwrap().unwrap(), Event::EndList);
        assert_eq!(parser.next().unwrap().unwrap(), Event::EndMap);
        assert!(parser.next().is_none());
    }
}
