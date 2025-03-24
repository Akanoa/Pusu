use foundationdb_tuple::{TupleDepth, TuplePack, VersionstampOffset};
use std::io::Write;

pub(crate) mod pusu {
    include!(concat!(env!("OUT_DIR"), "/pusu.rs"));
}

mod biscuit;
mod channel;
pub mod errors;
pub mod service;
pub mod storage;
pub mod test_utils;

/// Enum representing the various data prefixes used in the application.
///
/// These prefixes are used to distinguish between different types of records
/// stored in the database. Each prefix is associated with a unique `u64` value.
#[derive(Clone, Copy)]
#[repr(u64)]
pub enum DataPrefix {
    /// Prefix for channel records.
    Channel = 1,
}

impl TuplePack for DataPrefix {
    fn pack<W: Write>(
        &self,
        w: &mut W,
        tuple_depth: TupleDepth,
    ) -> std::io::Result<VersionstampOffset> {
        (*self as u64).pack(w, tuple_depth)
    }
}
