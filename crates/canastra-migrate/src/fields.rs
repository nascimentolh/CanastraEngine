//! Field access over decoded records that tracks what was consumed.

use std::str::FromStr;

use canastra_data::asset::AssetRef;
use canastra_data::text::Localized;
use l2_dat::{Record, Value};

pub(crate) type Result<T> = std::result::Result<T, String>;

/// The client writes empty, `none` or `[none]` for "no value".
fn is_blank(text: &str) -> bool {
    text.is_empty() || text.eq_ignore_ascii_case("none") || text.eq_ignore_ascii_case("[none]")
}

pub(crate) fn parse<T: FromStr>(name: &str, value: &str) -> Result<T> {
    value.trim().parse().map_err(|_| format!("invalid {name} `{value}`"))
}

/// A client code such as an animation or icon id, empty when blank.
pub(crate) fn code(text: &str) -> String {
    if is_blank(text) { String::new() } else { text.to_owned() }
}

pub(crate) fn localized(text: &str) -> Localized {
    if is_blank(text) { Localized::default() } else { Localized::en(text) }
}

/// Collects asset references and notes what had to be dropped or repaired.
#[derive(Default)]
pub(crate) struct Assets {
    pub(crate) notes: Vec<String>,
}

impl Assets {
    pub(crate) fn one<K>(&mut self, path: &str) -> Option<AssetRef<K>> {
        if is_blank(path) {
            return None;
        }
        AssetRef::parse(path).map_err(|error| self.notes.push(format!("dropped asset reference: {error}"))).ok()
    }

    pub(crate) fn many<K>(&mut self, paths: &[&str]) -> Vec<AssetRef<K>> {
        paths.iter().filter_map(|path| self.one(path)).collect()
    }
}

pub(crate) struct Fields<'a> {
    record: &'a Record,
    used: Vec<bool>,
}

impl<'a> Fields<'a> {
    pub(crate) fn of(value: &'a Value) -> Result<Self> {
        match value {
            Value::Record(record) => Ok(Self { record, used: vec![false; record.len()] }),
            other => Err(format!("expected a record, found {other:?}")),
        }
    }

    fn take(&mut self, name: &str) -> Result<&'a Value> {
        let record = self.record;
        let (index, (_, value)) = record
            .iter()
            .enumerate()
            .find(|&(index, (key, _))| *key == name && !self.used.get(index).copied().unwrap_or(true))
            .ok_or_else(|| format!("missing field `{name}`"))?;
        if let Some(used) = self.used.get_mut(index) {
            *used = true;
        }
        Ok(value)
    }

    pub(crate) fn uint(&mut self, name: &str) -> Result<u32> {
        match self.take(name)? {
            Value::Int(int) => u32::try_from(*int).map_err(|_| format!("`{name}` = {int} is out of range")),
            other => Err(format!("`{name}` should be an integer, found {other:?}")),
        }
    }

    pub(crate) fn int8(&mut self, name: &str) -> Result<i8> {
        match self.take(name)? {
            Value::Int(int) => i8::try_from(*int).map_err(|_| format!("`{name}` = {int} is out of range")),
            other => Err(format!("`{name}` should be an integer, found {other:?}")),
        }
    }

    pub(crate) fn float(&mut self, name: &str) -> Result<f32> {
        match self.take(name)? {
            Value::Float(float) => Ok(*float),
            other => Err(format!("`{name}` should be a float, found {other:?}")),
        }
    }

    /// A 0/1 integer.
    pub(crate) fn flag(&mut self, name: &str) -> Result<bool> {
        match self.uint(name)? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(format!("`{name}` = {other} is not 0 or 1")),
        }
    }

    pub(crate) fn text(&mut self, name: &str) -> Result<&'a str> {
        match self.take(name)? {
            Value::Text(text) => Ok(text),
            other => Err(format!("`{name}` should be text, found {other:?}")),
        }
    }

    pub(crate) fn list(&mut self, name: &str) -> Result<&'a [Value]> {
        match self.take(name)? {
            Value::List(items) => Ok(items),
            other => Err(format!("`{name}` should be a list, found {other:?}")),
        }
    }

    pub(crate) fn texts(&mut self, name: &str) -> Result<Vec<&'a str>> {
        self.list(name)?.iter().map(|value| text_of(value, name)).collect()
    }

    pub(crate) fn uints(&mut self, name: &str) -> Result<Vec<u32>> {
        self.list(name)?
            .iter()
            .map(|value| match value {
                Value::Int(int) => u32::try_from(*int).map_err(|_| format!("`{name}` item {int} is out of range")),
                other => Err(format!("`{name}` items should be integers, found {other:?}")),
            })
            .collect()
    }

    pub(crate) fn nested(&mut self, name: &str) -> Result<Fields<'a>> {
        Fields::of(self.take(name)?)
    }

    /// Every field of this record as text, in order.
    pub(crate) fn into_texts(self) -> Result<Vec<&'a str>> {
        self.record.iter().map(|(key, value)| text_of(value, key)).collect()
    }

    /// Consumes fields on purpose; names that are absent are fine.
    pub(crate) fn ignore(&mut self, names: &[&str]) {
        for (index, (key, _)) in self.record.iter().enumerate() {
            if names.contains(key)
                && let Some(used) = self.used.get_mut(index)
            {
                *used = true;
            }
        }
    }

    /// Fails if any field was neither mapped nor ignored.
    pub(crate) fn finish(self) -> Result<()> {
        let unused: Vec<&str> =
            self.record.iter().zip(&self.used).filter(|&(_, used)| !used).map(|((key, _), _)| *key).collect();
        match unused.as_slice() {
            [] => Ok(()),
            names => Err(format!("unmapped legacy fields: {}", names.join(", "))),
        }
    }
}

pub(crate) fn text_of<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    match value {
        Value::Text(text) => Ok(text),
        other => Err(format!("`{name}` items should be text, found {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Value {
        Value::Record(vec![
            ("id", Value::Int(7)),
            ("name", Value::Text("Sword".into())),
            ("junk", Value::Int(0)),
            ("new_field", Value::Int(1)),
        ])
    }

    #[test]
    fn finish_reports_unmapped_fields() {
        let value = record();
        let mut fields = Fields::of(&value).unwrap();
        assert_eq!(fields.uint("id").unwrap(), 7);
        assert_eq!(fields.text("name").unwrap(), "Sword");
        fields.ignore(&["junk", "absent"]);
        assert_eq!(fields.finish().unwrap_err(), "unmapped legacy fields: new_field");
    }

    #[test]
    fn type_and_presence_errors() {
        let value = record();
        let mut fields = Fields::of(&value).unwrap();
        assert!(fields.text("id").is_err());
        assert!(fields.uint("missing").is_err());
    }
}
