use crate::pusu;
use crate::pusu::response::*;
use crate::pusu::{ConnectionAcceptedResponse, FailResponse, MessageResponse, OkResponse};
use prost::Message;

pub fn create_auth_response() -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Response {
        response: Some(Response::Connection(ConnectionAcceptedResponse {})),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_ok_response() -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Response {
        response: Some(Response::Ok(OkResponse {})),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_fail_response(error: &str) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Response {
        response: Some(Response::Fail(FailResponse {
            error: error.to_string(),
        })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_message_response(message: Option<Vec<u8>>) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Response {
        response: Some(Response::Message(MessageResponse { message })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}
