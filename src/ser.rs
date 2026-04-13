use serde::ser::{self, Serialize};

use crate::error::Error;
use crate::Value;

pub fn to_string<T: Serialize>(value: &T) -> Result<String, Error> {
    let val = to_value(value)?;
    let Value::Map(pairs) = val else {
        unreachable!("to_value enforces top-level map");
    };
    Ok(format_map(&pairs, 0, true))
}

pub fn to_value<T: Serialize>(value: &T) -> Result<Value, Error> {
    let val = value.serialize(ValueSerializer)?;
    if !matches!(val, Value::Map(_)) {
        return Err(Error::new("top-level value must be a map"));
    }
    Ok(val)
}

fn format_value(v: &Value, indent: usize) -> String {
    match v {
        Value::String(s) => format_string(s),
        Value::List(items) => format_list(items, indent),
        Value::Map(pairs) => format_map(pairs, indent, false),
    }
}

fn format_string(s: &str) -> String {
    if can_be_bare(s) {
        s.to_string()
    } else {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '\0' => out.push_str("\\0"),
                c if c.is_control() => {
                    out.push_str(&format!("\\u{{{:x}}}", c as u32));
                }
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
}

fn can_be_bare(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            !c.is_whitespace() && !matches!(c, '{' | '}' | '[' | ']' | '"' | '#' | '\'' | ';')
        })
}

fn format_list(items: &[Value], indent: usize) -> String {
    if items.is_empty() {
        return "[]".to_string();
    }

    let has_compound = items
        .iter()
        .any(|v| matches!(v, Value::Map(_) | Value::List(_)));

    if has_compound {
        let inner = indent + 1;
        let pad = "  ".repeat(inner);
        let close_pad = "  ".repeat(indent);
        let parts: Vec<String> = items
            .iter()
            .map(|v| format!("{}{}", pad, format_value(v, inner)))
            .collect();
        format!("[\n{}\n{}]", parts.join("\n"), close_pad)
    } else {
        let parts: Vec<String> = items.iter().map(|v| format_value(v, indent)).collect();
        format!("[{}]", parts.join(" "))
    }
}

fn format_map(pairs: &[(String, Value)], indent: usize, top_level: bool) -> String {
    if pairs.is_empty() {
        return if top_level {
            String::new()
        } else {
            "{}".to_string()
        };
    }

    let mut out = String::new();
    let inner = if top_level { indent } else { indent + 1 };
    let pad = "  ".repeat(inner);

    if !top_level {
        out.push_str("{\n");
    }

    for (k, v) in pairs {
        out.push_str(&pad);
        out.push_str(&format_string(k));
        out.push(' ');
        out.push_str(&format_value(v, inner));
        out.push('\n');
    }

    if !top_level {
        out.push_str(&"  ".repeat(indent));
        out.push('}');
    }

    out
}

struct ValueSerializer;

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqCollector;
    type SerializeTuple = SeqCollector;
    type SerializeTupleStruct = SeqCollector;
    type SerializeTupleVariant = TupleVariantCollector;
    type SerializeMap = MapCollector;
    type SerializeStruct = MapCollector;
    type SerializeStructVariant = StructVariantCollector;

    fn serialize_bool(self, v: bool) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Value, Error> {
        Ok(Value::String(v.to_string()))
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<Value, Error> {
        Err(Error::new("byte arrays are not supported"))
    }

    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Value::String(String::new()))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Value, Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Value::String(String::new()))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Value::String(String::new()))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Value::String(variant.to_string()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        let inner = value.serialize(ValueSerializer)?;
        Ok(Value::Map(vec![(variant.to_string(), inner)]))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SeqCollector, Error> {
        Ok(SeqCollector {
            items: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqCollector, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<SeqCollector, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<TupleVariantCollector, Error> {
        Ok(TupleVariantCollector {
            variant: variant.to_string(),
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, len: Option<usize>) -> Result<MapCollector, Error> {
        Ok(MapCollector {
            pairs: Vec::with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<MapCollector, Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<StructVariantCollector, Error> {
        Ok(StructVariantCollector {
            variant: variant.to_string(),
            pairs: Vec::with_capacity(len),
        })
    }
}

struct SeqCollector {
    items: Vec<Value>,
}

impl ser::SerializeSeq for SeqCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::List(self.items))
    }
}

impl ser::SerializeTuple for SeqCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SeqCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Value, Error> {
        ser::SerializeSeq::end(self)
    }
}

struct TupleVariantCollector {
    variant: String,
    items: Vec<Value>,
}

impl ser::SerializeTupleVariant for TupleVariantCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(vec![(self.variant, Value::List(self.items))]))
    }
}

struct MapCollector {
    pairs: Vec<(String, Value)>,
    key: Option<String>,
}

impl ser::SerializeMap for MapCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        let val = key.serialize(ValueSerializer)?;
        self.key = Some(match val {
            Value::String(s) => s,
            _ => return Err(Error::new("map keys must be strings")),
        });
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .key
            .take()
            .ok_or_else(|| Error::new("serialize_value without key"))?;
        self.pairs.push((key, value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(self.pairs))
    }
}

impl ser::SerializeStruct for MapCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.pairs
            .push((key.to_string(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(self.pairs))
    }
}

struct StructVariantCollector {
    variant: String,
    pairs: Vec<(String, Value)>,
}

impl ser::SerializeStructVariant for StructVariantCollector {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.pairs
            .push((key.to_string(), value.serialize(ValueSerializer)?));
        Ok(())
    }

    fn end(self) -> Result<Value, Error> {
        Ok(Value::Map(vec![(self.variant, Value::Map(self.pairs))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[test]
    fn simple_struct() {
        #[derive(Serialize)]
        struct Config {
            name: String,
            port: u16,
        }

        let s = to_string(&Config {
            name: "app".into(),
            port: 8080,
        })
        .unwrap();
        assert_eq!(s, "name app\nport 8080\n");
    }

    #[test]
    fn nested_struct() {
        #[derive(Serialize)]
        struct Inner {
            host: String,
            port: u16,
        }

        #[derive(Serialize)]
        struct Config {
            server: Inner,
        }

        let s = to_string(&Config {
            server: Inner {
                host: "localhost".into(),
                port: 3000,
            },
        })
        .unwrap();
        assert_eq!(s, "server {\n  host localhost\n  port 3000\n}\n");
    }

    #[test]
    fn list_field() {
        #[derive(Serialize)]
        struct Config {
            tags: Vec<String>,
        }

        let s = to_string(&Config {
            tags: vec!["web".into(), "prod".into()],
        })
        .unwrap();
        assert_eq!(s, "tags [web prod]\n");
    }

    #[test]
    fn quoted_strings() {
        #[derive(Serialize)]
        struct Config {
            msg: String,
        }

        let s = to_string(&Config {
            msg: "hello world".into(),
        })
        .unwrap();
        assert_eq!(s, "msg \"hello world\"\n");
    }

    #[test]
    fn escape_sequences() {
        #[derive(Serialize)]
        struct Config {
            msg: String,
        }

        let s = to_string(&Config {
            msg: "line1\nline2".into(),
        })
        .unwrap();
        assert_eq!(s, "msg \"line1\\nline2\"\n");
    }

    #[test]
    fn bool_fields() {
        #[derive(Serialize)]
        struct Config {
            debug: bool,
        }

        let s = to_string(&Config { debug: true }).unwrap();
        assert_eq!(s, "debug true\n");
    }

    #[test]
    fn empty_map() {
        let s = to_string(&std::collections::HashMap::<String, String>::new()).unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn top_level_string_rejected() {
        assert!(to_string(&"hello").is_err());
    }

    #[test]
    fn top_level_list_rejected() {
        assert!(to_string(&vec!["a", "b"]).is_err());
    }

    #[test]
    fn empty_string_quoted() {
        #[derive(Serialize)]
        struct Config {
            name: String,
        }

        let s = to_string(&Config {
            name: String::new(),
        })
        .unwrap();
        assert_eq!(s, "name \"\"\n");
    }

    #[test]
    fn nbsp_quoted_roundtrip() {
        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
        }

        let cfg = Config {
            name: "foo\u{00a0}bar".into(),
        };
        let s = to_string(&cfg).unwrap();
        assert!(s.contains('"'), "nbsp value must be quoted");
        let cfg2: Config = crate::from_str(&s).unwrap();
        assert_eq!(cfg, cfg2);
    }
}
