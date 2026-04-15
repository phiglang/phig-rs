#[cfg(feature = "serde")]
pub(crate) mod de;
#[cfg(feature = "serde")]
pub(crate) mod ser;

use crate::error::Error;
use crate::parse::{Event, Parser};

/// A dynamically-typed phig value.
///
/// Phig has three types: strings, ordered lists, and ordered maps with
/// unique string keys. Use the accessor methods ([`as_str`](Value::as_str),
/// [`as_list`](Value::as_list), [`as_map`](Value::as_map)) or index
/// directly with `&str` / `usize`.
///
/// ```
/// let v: phig::Value = "name foo\ntags [a b]".parse().unwrap();
/// assert_eq!(v["name"].as_str(), Some("foo"));
/// assert_eq!(v["tags"][1].as_str(), Some("b"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A string.
    String(String),
    /// An ordered sequence of values.
    List(Vec<Value>),
    /// An ordered sequence of key-value pairs with unique string keys.
    Map(Vec<(String, Value)>),
}

impl Value {
    /// Returns the string contents if this is a `Value::String`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the items if this is a `Value::List`.
    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(v) => Some(v),
            _ => None,
        }
    }

    /// Returns the key-value pairs if this is a `Value::Map`.
    pub fn as_map(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Looks up a key in a map. Returns `None` if not a map or key is absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

impl std::str::FromStr for Value {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_to_value(s.as_bytes())
    }
}

/// Parse phig input from a reader into a [`Value`] tree.
pub(crate) fn parse_to_value(reader: impl std::io::Read) -> Result<Value, Error> {
    enum Frame {
        Map {
            pairs: Vec<(String, Value)>,
            pending_key: Option<String>,
        },
        List {
            items: Vec<Value>,
        },
    }

    let mut parser = Parser::new(reader);
    let mut stack: Vec<Frame> = Vec::new();
    let mut result: Option<Value> = None;

    fn push_value(stack: &mut Vec<Frame>, result: &mut Option<Value>, value: Value) {
        match stack.last_mut() {
            Some(Frame::Map { pairs, pending_key }) => {
                pairs.push((pending_key.take().expect("map value without key"), value));
            }
            Some(Frame::List { items }) => items.push(value),
            None => *result = Some(value),
        }
    }

    for event in &mut parser {
        let event = event?;
        match event {
            Event::StartMap => stack.push(Frame::Map {
                pairs: Vec::new(),
                pending_key: None,
            }),
            Event::EndMap => {
                let Frame::Map { pairs, .. } = stack.pop().expect("unbalanced EndMap") else {
                    panic!("EndMap on non-map frame");
                };
                push_value(&mut stack, &mut result, Value::Map(pairs));
            }
            Event::StartList => stack.push(Frame::List { items: Vec::new() }),
            Event::EndList => {
                let Frame::List { items } = stack.pop().expect("unbalanced EndList") else {
                    panic!("EndList on non-list frame");
                };
                push_value(&mut stack, &mut result, Value::List(items));
            }
            Event::Key(k) => match stack.last_mut() {
                Some(Frame::Map { pending_key, .. }) => {
                    *pending_key = Some(k);
                }
                _ => panic!("Key outside of map"),
            },
            Event::String(s) => {
                push_value(&mut stack, &mut result, Value::String(s));
            }
        }
    }

    Ok(result.expect("no value produced"))
}

impl<'a> std::ops::Index<&'a str> for Value {
    type Output = Value;

    fn index(&self, key: &'a str) -> &Value {
        self.get(key)
            .unwrap_or_else(|| panic!("key not found: {}", key))
    }
}

impl std::ops::Index<usize> for Value {
    type Output = Value;

    fn index(&self, idx: usize) -> &Value {
        match self {
            Value::List(items) => &items[idx],
            _ => panic!("not a list"),
        }
    }
}
