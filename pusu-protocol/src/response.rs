use crate::pusu;
use crate::pusu::response::*;
use crate::pusu::{ConnectionAcceptedResponse, FailResponse, MessageResponse, OkResponse};
use prost::Message;

pub fn create_auth_response_struct() -> pusu::Response {
    pusu::Response {
        response: Some(Response::Connection(ConnectionAcceptedResponse {})),
    }
}

pub fn create_auth_response() -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    create_auth_response_struct().encode(&mut buf)?;
    Ok(buf)
}

pub fn create_ok_response_struct() -> pusu::Response {
    pusu::Response {
        response: Some(Response::Ok(OkResponse {})),
    }
}

pub fn create_ok_response() -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    create_ok_response_struct().encode(&mut buf)?;
    Ok(buf)
}

pub fn create_fail_response_struct(error: &str) -> pusu::Response {
    pusu::Response {
        response: Some(Response::Fail(FailResponse {
            error: error.to_string(),
        })),
    }
}

pub fn create_fail_response(error: &str) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    create_fail_response_struct(error).encode(&mut buf)?;
    Ok(buf)
}

pub fn create_message_response_struct(message: Option<Vec<u8>>) -> pusu::Response {
    pusu::Response {
        response: Some(Response::Message(MessageResponse { message })),
    }
}

pub fn create_message_response(message: Option<Vec<u8>>) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    create_message_response_struct(message).encode(&mut buf)?;
    Ok(buf)
}
