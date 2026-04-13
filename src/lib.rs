//! Parser and serializer for the [phig](https://phiglang.github.io) configuration language.
//!
//! Phig has three types: strings, lists, and maps. Type coercion (to numbers,
//! booleans, etc.) is handled automatically via serde.
//!
//! # Examples
//!
//! Deserialize into a struct:
//!
//! ```
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Config {
//!     name: String,
//!     port: u16,
//!     tags: Vec<String>,
//! }
//!
//! let cfg: Config = phig::from_str("name app\nport 8080\ntags [web prod]").unwrap();
//! assert_eq!(cfg.name, "app");
//! assert_eq!(cfg.port, 8080);
//! ```
//!
//! Work with untyped values:
//!
//! ```
//! let val: phig::Value = "name foo\ntags [a b c]".parse().unwrap();
//! assert_eq!(val["name"].as_str(), Some("foo"));
//! assert_eq!(val["tags"][0].as_str(), Some("a"));
//! ```

mod de;
mod error;
mod parse;
mod ser;

pub use de::{from_reader, from_str, from_value};
pub use error::Error;
pub use ser::{to_string, to_value, to_writer};

use std::fmt;

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
        parse::parse(s.as_bytes())
    }
}

impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self {
            Value::String(s) => serializer.serialize_str(s),
            Value::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::Map(pairs) => {
                let mut map = serializer.serialize_map(Some(pairs.len()))?;
                for (k, v) in pairs {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> serde::de::Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a phig value")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(v) = seq.next_element()? {
            items.push(v);
        }
        Ok(Value::List(items))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut pairs = Vec::new();
        while let Some((k, v)) = map.next_entry()? {
            pairs.push((k, v));
        }
        Ok(Value::Map(pairs))
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Tls {
        cert: String,
        key: String,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Server {
        host: String,
        port: u16,
        tls: Tls,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Pool {
        min: u32,
        max: u32,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Db {
        url: String,
        pool: Pool,
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct AppConfig {
        name: String,
        version: String,
        tags: Vec<String>,
        server: Server,
        db: Db,
    }

    #[test]
    fn full_roundtrip() {
        let input = r#"
name "My App"
version 1.0.2
tags [web production v2]
server {
  host 0.0.0.0
  port 8080
  tls {
    cert /etc/ssl/cert.pem
    key /etc/ssl/key.pem
  }
}
db {
  url "postgres://localhost/primary"
  pool { min 2; max 10 }
}
"#;

        let config: AppConfig = from_str(input).unwrap();
        assert_eq!(config.name, "My App");
        assert_eq!(config.version, "1.0.2");
        assert_eq!(config.tags, vec!["web", "production", "v2"]);
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.tls.cert, "/etc/ssl/cert.pem");
        assert_eq!(config.db.pool.min, 2);
        assert_eq!(config.db.pool.max, 10);

        let serialized = to_string(&config).unwrap();
        let config2: AppConfig = from_str(&serialized).unwrap();
        assert_eq!(config, config2);
    }

    #[test]
    fn value_roundtrip() {
        let input = "name foo\ntags [a b c]\nnested { x 1; y 2 }";
        let v: Value = from_str(input).unwrap();

        assert_eq!(v["name"].as_str(), Some("foo"));
        assert_eq!(v["tags"][0].as_str(), Some("a"));
        assert_eq!(v["nested"]["x"].as_str(), Some("1"));

        let serialized = to_string(&v).unwrap();
        let v2: Value = from_str(&serialized).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn enum_variants() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum LogLevel {
            Debug,
            Info,
            Warn,
            Error,
        }

        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct LogConfig {
            level: LogLevel,
            format: String,
        }

        let input = r#"level debug; format "%t %msg""#;
        let cfg: LogConfig = from_str(input).unwrap();
        assert_eq!(cfg.level, LogLevel::Debug);
        assert_eq!(cfg.format, "%t %msg");

        let serialized = to_string(&cfg).unwrap();
        let cfg2: LogConfig = from_str(&serialized).unwrap();
        assert_eq!(cfg, cfg2);
    }

    #[test]
    fn optional_fields() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Config {
            name: String,
            #[serde(default)]
            debug: Option<bool>,
            #[serde(default)]
            workers: Option<u32>,
        }

        let input = "name app\nworkers 4";
        let cfg: Config = from_str(input).unwrap();
        assert_eq!(cfg.name, "app");
        assert_eq!(cfg.debug, None);
        assert_eq!(cfg.workers, Some(4));
    }

    #[test]
    fn special_strings() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Config {
            greeting: String,
            path: String,
        }

        let input = "greeting \"hello\\tworld\\n\"\npath 'C:\\Users\\test'";
        let cfg: Config = from_str(input).unwrap();
        assert_eq!(cfg.greeting, "hello\tworld\n");
        assert_eq!(cfg.path, "C:\\Users\\test");
    }

    #[test]
    fn comments_ignored() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Config {
            name: String,
            port: u16,
        }

        let input = "# main config\nname app # the app name\nport 3000\n# end";
        let cfg: Config = from_str(input).unwrap();
        assert_eq!(cfg.name, "app");
        assert_eq!(cfg.port, 3000);
    }

    #[test]
    fn floats() {
        #[derive(Debug, PartialEq, Serialize, Deserialize)]
        struct Config {
            rate: f64,
        }

        let cfg: Config = from_str("rate 3.14").unwrap();
        assert!((cfg.rate - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn from_str_value_parse() {
        let v: Value = "name foo\ncount 42".parse().unwrap();
        assert_eq!(v["name"].as_str(), Some("foo"));
        assert_eq!(v["count"].as_str(), Some("42"));
    }
}
