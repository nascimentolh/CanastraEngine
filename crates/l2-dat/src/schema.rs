//! Declarative layout of a client `.dat` table.

/// One on-disk field. Integer fields are remembered by name so later
/// arrays and conditions can refer to them.
#[derive(Debug, Clone, Copy)]
pub struct Field {
    pub name: &'static str,
    pub kind: Kind,
    /// Only sizes an array; not emitted in the decoded record.
    pub counter: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Scalar(Scalar),
    Group(&'static [Field]),
    Array(Len, &'static [Field]),
    /// Fields read only when an earlier integer field equals `equals`; flattened into the parent.
    If {
        param: &'static str,
        equals: i64,
        fields: &'static [Field],
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Scalar {
    /// Signed byte.
    I8,
    U8,
    I32,
    U32,
    F32,
    /// Four bytes, stored as `0xAARRGGBB`.
    Rgba,
    /// UE2 compact index.
    Compact,
    /// `i32` byte length + `UTF-16LE`.
    Unicode,
    /// UE2 `FString`.
    Ascf,
}

pub const I8: Kind = Kind::Scalar(Scalar::I8);
pub const U8: Kind = Kind::Scalar(Scalar::U8);
pub const I32: Kind = Kind::Scalar(Scalar::I32);
pub const U32: Kind = Kind::Scalar(Scalar::U32);
pub const F32: Kind = Kind::Scalar(Scalar::F32);
pub const RGBA: Kind = Kind::Scalar(Scalar::Rgba);
pub const COMPACT: Kind = Kind::Scalar(Scalar::Compact);
pub const UNICODE: Kind = Kind::Scalar(Scalar::Unicode);
pub const ASCF: Kind = Kind::Scalar(Scalar::Ascf);

#[derive(Debug, Clone, Copy)]
pub enum Len {
    Fixed(u32),
    Field(&'static str),
}

#[derive(Debug)]
pub struct Table {
    /// Lower-case file stem, e.g. `armorgrp`.
    pub name: &'static str,
    /// Stem carries a language suffix, e.g. `itemname-e`.
    pub localized: bool,
    pub fields: &'static [Field],
}

impl Table {
    pub fn matches(&self, file_name: &str) -> bool {
        let lower = file_name.to_ascii_lowercase();
        let Some(stem) = lower.strip_suffix(".dat") else {
            return false;
        };
        if stem == self.name {
            return !self.localized;
        }
        self.localized
            && stem
                .strip_prefix(self.name)
                .and_then(|rest| rest.strip_prefix('-'))
                .is_some_and(|lang| !lang.is_empty() && lang.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    }
}

pub const fn f(name: &'static str, kind: Kind) -> Field {
    Field { name, kind, counter: false }
}

pub const fn counter(name: &'static str, kind: Kind) -> Field {
    Field { name, kind, counter: true }
}
