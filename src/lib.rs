//! Parser and serializer for the [phig](https://phiglang.github.io) configuration language.
//!
//! Phig has three types: strings, lists, and maps. When the `serde` feature is
//! enabled (default), type coercion to numbers, booleans, etc. is handled
//! automatically.
//!
//! # Examples
//!
//! Work with untyped values:
//!
//! ```
//! let val: phig::Value = "name foo\ntags [a b c]".parse().unwrap();
//! assert_eq!(val["name"].as_str(), Some("foo"));
//! assert_eq!(val["tags"][0].as_str(), Some("a"));
//! ```
//!
//! Deserialize into a struct (requires `serde` feature):
//!
//! ```
//! # #[cfg(feature = "serde")] {
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
//! # }
//! ```

#[cfg(feature = "serde")]
mod de;
mod error;
pub mod fmt;
pub mod parse;
#[cfg(feature = "serde")]
mod ser;

#[cfg(feature = "serde")]
pub use de::{from_reader, from_str, from_value};
pub use error::Error;
#[cfg(feature = "serde")]
pub use ser::{to_string, to_value, to_writer};

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
        let mut builder = ValueBuilder::new();
        parse::parse(s.as_bytes(), &mut builder)?;
        Ok(builder.finish())
    }
}

struct ValueBuilder {
    stack: Vec<BuildFrame>,
    result: Option<Value>,
}

enum BuildFrame {
    Map {
        pairs: Vec<(String, Value)>,
        pending_key: Option<String>,
    },
    List {
        items: Vec<Value>,
    },
}

impl ValueBuilder {
    fn new() -> Self {
        ValueBuilder {
            stack: Vec::new(),
            result: None,
        }
    }

    fn finish(self) -> Value {
        self.result.expect("no value produced")
    }

    fn push_value(&mut self, value: Value) {
        match self.stack.last_mut() {
            Some(BuildFrame::Map { pairs, pending_key }) => {
                let key = pending_key.take().expect("map value without key");
                pairs.push((key, value));
            }
            Some(BuildFrame::List { items }) => {
                items.push(value);
            }
            None => {
                self.result = Some(value);
            }
        }
    }
}

impl parse::Handler for ValueBuilder {
    type Error = Error;

    fn map_start(&mut self) -> Result<(), Error> {
        self.stack.push(BuildFrame::Map {
            pairs: Vec::new(),
            pending_key: None,
        });
        Ok(())
    }

    fn map_end(&mut self) -> Result<(), Error> {
        let frame = self.stack.pop().expect("unbalanced map_end");
        let value = match frame {
            BuildFrame::Map { pairs, .. } => Value::Map(pairs),
            _ => panic!("map_end on non-map frame"),
        };
        self.push_value(value);
        Ok(())
    }

    fn list_start(&mut self) -> Result<(), Error> {
        self.stack.push(BuildFrame::List { items: Vec::new() });
        Ok(())
    }

    fn list_end(&mut self) -> Result<(), Error> {
        let frame = self.stack.pop().expect("unbalanced list_end");
        let value = match frame {
            BuildFrame::List { items } => Value::List(items),
            _ => panic!("list_end on non-list frame"),
        };
        self.push_value(value);
        Ok(())
    }

    fn key(&mut self, key: String) -> Result<(), Error> {
        match self.stack.last_mut() {
            Some(BuildFrame::Map { pending_key, .. }) => {
                *pending_key = Some(key);
            }
            _ => panic!("key outside of map"),
        }
        Ok(())
    }

    fn string(&mut self, value: String) -> Result<(), Error> {
        self.push_value(Value::String(value));
        Ok(())
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

#[cfg(all(test, feature = "serde"))]
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
