//! Schema-driven decoder for decrypted Lineage 2 `.dat` tables (see `l2-crypto`).
//!
//! This layer exists to migrate the legacy `system` tables; field names follow
//! the on-disk layout, not the game's domain model.

#[rustfmt::skip]
mod h5;
pub mod schema;

use std::collections::HashMap;
use std::fmt;

use schema::{Field, Kind, Len, Scalar, Table};
use ue2_core::{ReadError, Reader};

pub use h5::TABLES as H5_TABLES;

/// `FString` `SafePackage` written after the last record.
const SAFE_PACKAGE: &[u8] = b"\x0cSafePackage\0";

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f32),
    Text(String),
    Record(Record),
    List(Vec<Value>),
}

pub type Record = Vec<(&'static str, Value)>;

#[derive(Debug)]
pub enum Error {
    Read { field: &'static str, source: ReadError },
    MissingCounter(&'static str),
    BadCount { field: &'static str, count: i64 },
    TrailingBytes { offset: usize, len: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { field, source } => write!(f, "field `{field}`: {source}"),
            Self::MissingCounter(name) => write!(f, "array size `{name}` was never read"),
            Self::BadCount { field, count } => write!(f, "array `{field}` has impossible size {count}"),
            Self::TrailingBytes { offset, len } => write!(f, "{len} undecoded bytes at {offset:#x}"),
        }
    }
}

impl std::error::Error for Error {}

/// The H5 table layout for a file name such as `ItemName-e.dat`.
pub fn h5_table(file_name: &str) -> Option<&'static Table> {
    H5_TABLES.iter().find(|table| table.matches(file_name))
}

/// Decodes a whole table; every byte must belong to a field or the trailing marker.
pub fn decode(table: &Table, data: &[u8]) -> Result<Record, Error> {
    let mut decoder = Decoder { reader: Reader::at(data, 0), vars: HashMap::new() };
    let mut record = Record::new();
    decoder.fields(table.fields, &mut record)?;
    let rest = decoder.reader.remaining();
    if !rest.is_empty() && rest != SAFE_PACKAGE {
        return Err(Error::TrailingBytes { offset: decoder.reader.pos(), len: rest.len() });
    }
    Ok(record)
}

struct Decoder<'a> {
    reader: Reader<'a>,
    vars: HashMap<&'static str, i64>,
}

impl Decoder<'_> {
    fn fields(&mut self, fields: &'static [Field], out: &mut Record) -> Result<(), Error> {
        for field in fields {
            let value = match field.kind {
                Kind::Group(inner) => {
                    let mut record = Record::new();
                    self.fields(inner, &mut record)?;
                    Value::Record(record)
                }
                Kind::Array(len, inner) => self.array(field.name, len, inner)?,
                Kind::If { param, equals, fields: inner } => {
                    if self.vars.get(param) == Some(&equals) {
                        self.fields(inner, out)?;
                    }
                    continue;
                }
                Kind::Scalar(scalar) => {
                    self.scalar(scalar).map_err(|source| Error::Read { field: field.name, source })?
                }
            };
            if let Value::Int(int) = value {
                self.vars.insert(field.name, int);
            }
            if !field.counter {
                out.push((field.name, value));
            }
        }
        Ok(())
    }

    fn array(&mut self, name: &'static str, len: Len, fields: &'static [Field]) -> Result<Value, Error> {
        let count = match len {
            Len::Fixed(n) => i64::from(n),
            Len::Field(counter) => *self.vars.get(counter).ok_or(Error::MissingCounter(counter))?,
        };
        // Every element takes at least one byte, which bounds hostile sizes.
        let count = usize::try_from(count)
            .ok()
            .filter(|&n| n <= self.reader.remaining().len())
            .ok_or(Error::BadCount { field: name, count })?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let mut record = Record::new();
            self.fields(fields, &mut record)?;
            items.push(match <[_; 1]>::try_from(record) {
                Ok([(_, single)]) => single,
                Err(record) => Value::Record(record),
            });
        }
        Ok(Value::List(items))
    }

    fn scalar(&mut self, scalar: Scalar) -> Result<Value, ReadError> {
        let r = &mut self.reader;
        Ok(match scalar {
            Scalar::I8 => Value::Int(r.u8()?.cast_signed().into()),
            Scalar::U8 => Value::Int(r.u8()?.into()),
            Scalar::I32 => Value::Int(r.i32()?.into()),
            Scalar::U32 => Value::Int(r.u32()?.into()),
            Scalar::F32 => Value::Float(r.f32()?),
            Scalar::Rgba => Value::Int(u32::from_be_bytes(r.array()?).into()),
            Scalar::Compact => Value::Int(r.compact()?.into()),
            Scalar::Unicode => {
                let len = r.i32()?;
                Value::Text(r.utf16(usize::try_from(len).unwrap_or(0))?)
            }
            Scalar::Ascf => Value::Text(r.string()?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::schema::{ASCF, F32, I8, U8, U32, UNICODE, counter, f};
    use super::*;

    const ITEM: &[Field] = &[
        f("id", U32),
        f("name", UNICODE),
        counter("n", U8),
        f("tags", Kind::Array(Len::Field("n"), &[f("tag", ASCF)])),
        f("kind", I8),
        f("", Kind::If { param: "kind", equals: -1, fields: &[f("extra", F32)] }),
    ];
    const TABLE: Table = Table {
        name: "test",
        localized: true,
        fields: &[counter("count", U32), f("item", Kind::Array(Len::Field("count"), ITEM))],
    };

    #[test]
    fn decodes_nested_records_and_conditions() {
        let mut data = vec![2, 0, 0, 0];
        data.extend([7, 0, 0, 0, 4, 0, 0, 0, b'h', 0, b'i', 0, 1, 2, b'a', 0, 0xFF, 0, 0, 0x80, 0x3F]);
        data.extend([8, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        data.extend(SAFE_PACKAGE);

        let record = decode(&TABLE, &data).unwrap();
        let [(_, Value::List(items))] = record.as_slice() else { panic!("{record:?}") };
        assert_eq!(
            items[0],
            Value::Record(vec![
                ("id", Value::Int(7)),
                ("name", Value::Text("hi".into())),
                ("tags", Value::List(vec![Value::Text("a".into())])),
                ("kind", Value::Int(-1)),
                ("extra", Value::Float(1.0)),
            ])
        );
        assert_eq!(
            items[1],
            Value::Record(vec![
                ("id", Value::Int(8)),
                ("name", Value::Text(String::new())),
                ("tags", Value::List(vec![])),
                ("kind", Value::Int(1)),
            ])
        );
    }

    #[test]
    fn rejects_leftover_bytes_and_hostile_sizes() {
        assert!(matches!(decode(&TABLE, &[0, 0, 0, 0, 9]), Err(Error::TrailingBytes { offset: 4, len: 1 })));
        assert!(matches!(decode(&TABLE, &[0xFF, 0xFF, 0, 0]), Err(Error::BadCount { .. })));
    }

    #[test]
    fn table_names() {
        assert!(TABLE.matches("Test-e.dat") && TABLE.matches("test-ru.DAT"));
        assert!(!TABLE.matches("test.dat") && !TABLE.matches("test-e-original.dat"));
    }
}
