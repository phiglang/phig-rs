//! Phig formatter for writing values as phig text.

use std::io;

use crate::error::Error;
use crate::Value;

/// A phig formatter.
///
/// Can format a complete [`Value`] tree via [`write_value`](Formatter::write_value),
/// or be driven incrementally with [`map_start`](Formatter::map_start),
/// [`key`](Formatter::key), [`string`](Formatter::string), etc.
///
/// Lists are buffered internally to decide between inline (`[a b c]`)
/// and multiline formatting.
pub struct Formatter<W> {
    writer: W,
    stack: Vec<FormatFrame>,
}

enum FormatFrame {
    Map {
        indent: usize,
        inner_indent: usize,
        top_level: bool,
        opened: bool,
        pending_key: bool,
    },
    List {
        indent: usize,
        stack: Vec<ListBuildFrame>,
        nesting: usize,
    },
}

enum ListBuildFrame {
    List(Vec<Value>),
    Map(Vec<(String, Value)>, Option<String>),
}

fn list_buf_push(stack: &mut Vec<ListBuildFrame>, value: Value) {
    match stack.last_mut().unwrap() {
        ListBuildFrame::List(items) => items.push(value),
        ListBuildFrame::Map(pairs, key) => {
            pairs.push((key.take().expect("map value without key"), value));
        }
    }
}

impl<W> Formatter<W> {
    /// Create a new formatter that writes to `writer`.
    pub fn new(writer: W) -> Self {
        Formatter {
            writer,
            stack: Vec::new(),
        }
    }

    /// Consume the formatter and return the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: io::Write> Formatter<W> {
    /// Format a [`Value`] as top-level phig output.
    ///
    /// The value must be a [`Value::Map`].
    pub fn write_value(&mut self, value: &Value) -> Result<(), Error> {
        let Value::Map(pairs) = value else {
            return Err(Error::new("top-level value must be a map"));
        };
        format_map(pairs, 0, true, &mut self.writer)
    }

    fn value_written(&mut self) -> Result<(), Error> {
        let needs_nl = if let Some(FormatFrame::Map { pending_key, .. }) = self.stack.last_mut() {
            if *pending_key {
                *pending_key = false;
                true
            } else {
                false
            }
        } else {
            false
        };
        if needs_nl {
            self.writer.write_all(b"\n")?;
        }
        Ok(())
    }

    /// Begin a map. Call [`key`](Self::key) and a value method for each entry,
    /// then [`map_end`](Self::map_end) to close.
    pub fn map_start(&mut self) -> Result<(), Error> {
        if let Some(FormatFrame::List { nesting, stack, .. }) = self.stack.last_mut() {
            *nesting += 1;
            stack.push(ListBuildFrame::Map(Vec::new(), None));
            return Ok(());
        }

        let (indent, top_level) =
            if let Some(FormatFrame::Map { inner_indent, .. }) = self.stack.last() {
                (*inner_indent, false)
            } else {
                (0, true)
            };

        let inner_indent = if top_level { indent } else { indent + 1 };

        self.stack.push(FormatFrame::Map {
            indent,
            inner_indent,
            top_level,
            opened: false,
            pending_key: false,
        });
        Ok(())
    }

    /// End the current map.
    pub fn map_end(&mut self) -> Result<(), Error> {
        if let Some(FormatFrame::List { nesting, stack, .. }) = self.stack.last_mut() {
            assert!(*nesting > 0, "unbalanced map_end inside list");
            *nesting -= 1;
            let ListBuildFrame::Map(pairs, _) = stack.pop().unwrap() else {
                panic!("map_end on non-map frame");
            };
            list_buf_push(stack, Value::Map(pairs));
            return Ok(());
        }

        let frame = self.stack.pop().expect("unbalanced map_end");
        let FormatFrame::Map {
            indent,
            top_level,
            opened,
            ..
        } = frame
        else {
            panic!("map_end on non-map frame");
        };

        if !top_level {
            if opened {
                write_indent(&mut self.writer, indent)?;
                self.writer.write_all(b"}")?;
            } else {
                self.writer.write_all(b"{}")?;
            }
        }

        self.value_written()
    }

    /// Begin a list. Call value methods for each element, then
    /// [`list_end`](Self::list_end) to close.
    pub fn list_start(&mut self) -> Result<(), Error> {
        if let Some(FormatFrame::List { nesting, stack, .. }) = self.stack.last_mut() {
            *nesting += 1;
            stack.push(ListBuildFrame::List(Vec::new()));
            return Ok(());
        }

        let indent = if let Some(FormatFrame::Map { inner_indent, .. }) = self.stack.last() {
            *inner_indent
        } else {
            0
        };

        self.stack.push(FormatFrame::List {
            indent,
            stack: vec![ListBuildFrame::List(Vec::new())],
            nesting: 0,
        });
        Ok(())
    }

    /// End the current list.
    pub fn list_end(&mut self) -> Result<(), Error> {
        if let Some(FormatFrame::List { nesting, stack, .. }) = self.stack.last_mut() {
            if *nesting > 0 {
                *nesting -= 1;
                let ListBuildFrame::List(items) = stack.pop().unwrap() else {
                    panic!("list_end on non-list frame");
                };
                list_buf_push(stack, Value::List(items));
                return Ok(());
            }
        }

        let frame = self.stack.pop().expect("unbalanced list_end");
        let FormatFrame::List {
            indent, mut stack, ..
        } = frame
        else {
            panic!("list_end on non-list frame");
        };

        let ListBuildFrame::List(items) = stack.pop().unwrap() else {
            panic!("list_end on non-list frame");
        };
        format_list(&items, indent, &mut self.writer)?;
        self.value_written()
    }

    /// Emit a map key. Must be followed by exactly one value method call.
    pub fn key(&mut self, k: String) -> Result<(), Error> {
        if let Some(FormatFrame::List { stack, .. }) = self.stack.last_mut() {
            let ListBuildFrame::Map(_, key) = stack.last_mut().unwrap() else {
                panic!("key outside of map");
            };
            *key = Some(k);
            return Ok(());
        }

        let frame = self.stack.last_mut().expect("key outside of map");
        let FormatFrame::Map {
            opened,
            inner_indent,
            top_level,
            pending_key,
            ..
        } = frame
        else {
            panic!("key on non-map frame");
        };

        let need_open = !*top_level && !*opened;
        let ii = *inner_indent;
        *pending_key = true;
        if need_open {
            *opened = true;
        }

        if need_open {
            self.writer.write_all(b"{\n")?;
        }
        write_indent(&mut self.writer, ii)?;
        format_string(&k, &mut self.writer)?;
        self.writer.write_all(b" ")?;
        Ok(())
    }

    /// Emit a string value.
    pub fn string(&mut self, value: String) -> Result<(), Error> {
        if let Some(FormatFrame::List { stack, .. }) = self.stack.last_mut() {
            list_buf_push(stack, Value::String(value));
            return Ok(());
        }

        format_string(&value, &mut self.writer)?;
        self.value_written()
    }
}

fn format_value(v: &Value, indent: usize, w: &mut dyn io::Write) -> Result<(), Error> {
    match v {
        Value::String(s) => format_string(s, w),
        Value::List(items) => format_list(items, indent, w),
        Value::Map(pairs) => format_map(pairs, indent, false, w),
    }
}

fn format_string(s: &str, w: &mut dyn io::Write) -> Result<(), Error> {
    if can_be_bare(s) {
        w.write_all(s.as_bytes())?;
    } else {
        w.write_all(b"\"")?;
        for c in s.chars() {
            match c {
                '"' => w.write_all(b"\\\"")?,
                '\\' => w.write_all(b"\\\\")?,
                '\n' => w.write_all(b"\\n")?,
                '\r' => w.write_all(b"\\r")?,
                '\t' => w.write_all(b"\\t")?,
                '\0' => w.write_all(b"\\0")?,
                c if c.is_control() => write!(w, "\\u{{{:x}}}", c as u32)?,
                c => {
                    let mut buf = [0u8; 4];
                    w.write_all(c.encode_utf8(&mut buf).as_bytes())?;
                }
            }
        }
        w.write_all(b"\"")?;
    }
    Ok(())
}

fn can_be_bare(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            !c.is_whitespace() && !matches!(c, '{' | '}' | '[' | ']' | '"' | '#' | '\'' | ';')
        })
}

fn write_indent(w: &mut dyn io::Write, level: usize) -> Result<(), Error> {
    for _ in 0..level {
        w.write_all(b"  ")?;
    }
    Ok(())
}

fn format_list(items: &[Value], indent: usize, w: &mut dyn io::Write) -> Result<(), Error> {
    if items.is_empty() {
        w.write_all(b"[]")?;
        return Ok(());
    }

    let has_compound = items
        .iter()
        .any(|v| matches!(v, Value::Map(_) | Value::List(_)));

    if has_compound {
        let inner = indent + 1;
        w.write_all(b"[\n")?;
        for item in items {
            write_indent(w, inner)?;
            format_value(item, inner, w)?;
            w.write_all(b"\n")?;
        }
        write_indent(w, indent)?;
        w.write_all(b"]")?;
    } else {
        w.write_all(b"[")?;
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                w.write_all(b" ")?;
            }
            format_value(item, indent, w)?;
        }
        w.write_all(b"]")?;
    }
    Ok(())
}

fn format_map(
    pairs: &[(String, Value)],
    indent: usize,
    top_level: bool,
    w: &mut dyn io::Write,
) -> Result<(), Error> {
    if pairs.is_empty() {
        if !top_level {
            w.write_all(b"{}")?;
        }
        return Ok(());
    }

    let inner = if top_level { indent } else { indent + 1 };

    if !top_level {
        w.write_all(b"{\n")?;
    }

    for (k, v) in pairs {
        write_indent(w, inner)?;
        format_string(k, w)?;
        w.write_all(b" ")?;
        format_value(v, inner, w)?;
        w.write_all(b"\n")?;
    }

    if !top_level {
        write_indent(w, indent)?;
        w.write_all(b"}")?;
    }

    Ok(())
}
