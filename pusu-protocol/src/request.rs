use crate::pusu;
use crate::pusu::request::*;
use crate::pusu::{
    AuthRequest, ConsumeRequest, PublishRequest, QuitRequest, SubscribeRequest, UnsubscribeRequest,
};
use prost::Message;

pub fn create_auth_request(biscuit: &str) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Request {
        request: Some(Request::Auth(AuthRequest {
            biscuit: biscuit.to_string(),
        })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_quit_request() -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Request {
        request: Some(Request::Quit(QuitRequest {})),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_subscribe_request(channel_name: &str) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Request {
        request: Some(Request::Subscribe(SubscribeRequest {
            channel: channel_name.to_string(),
        })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_unsubscribe_request(channel_name: &str) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Request {
        request: Some(Request::Unsubscribe(UnsubscribeRequest {
            channel: channel_name.to_string(),
        })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_publish_request(
    channel_name: &str,
    message: &[u8],
) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Request {
        request: Some(Request::Publish(PublishRequest {
            channel: channel_name.to_string(),
            message: message.to_vec(),
        })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}

pub fn create_consume_request(channel_name: &str) -> crate::errors::Result<Vec<u8>> {
    let mut buf = Vec::new();
    pusu::Request {
        request: Some(Request::Consume(ConsumeRequest {
            channel: channel_name.to_string(),
        })),
    }
    .encode(&mut buf)?;
    Ok(buf)
}
