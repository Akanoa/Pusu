use foundationdb_tuple::{TupleDepth, TuplePack, VersionstampOffset};
use std::io::Write;

mod biscuit;
mod channel;
pub mod errors;
pub mod service;
pub mod storage;
pub mod test_utils;

#[derive(Clone, Copy)]
#[repr(u64)]
pub enum DataPrefix {
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
