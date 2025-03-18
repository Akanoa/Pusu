pub mod pusu {
    include!(concat!(env!("OUT_DIR"), "/pusu.rs"));
}

pub mod errors;
pub mod request;
pub mod response;
