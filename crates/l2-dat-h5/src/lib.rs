//! High Five `.dat` table layouts for `l2-dat`.
//!
//! Licensing boundary: 53 of these layouts derive from the GPL-licensed `L2ClientDat`
//! descriptors (`RideData` was derived from the client file). Only migration tooling may
//! depend on this crate; the client, server and Studio must not.

#[rustfmt::skip]
mod tables;

use l2_dat::schema::Table;

pub use tables::TABLES;

/// The layout for a file name such as `ItemName-e.dat`.
pub fn table(file_name: &str) -> Option<&'static Table> {
    TABLES.iter().find(|table| table.matches(file_name))
}
