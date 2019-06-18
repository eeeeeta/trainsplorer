//! Shared common library for inter-service communications.

use serde::{Serialize};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;
use nng::{Socket, Protocol};
use errors::{Result, ProtoError};
use wire::RpcResponse;
use failure::Fail;

pub static API_VERSION: u8 = 0;
pub type StdResult<T, E> = ::std::result::Result<T, E>;

pub mod wire;
pub mod errors;

/// An RPC interface for one of the trainsplorer microservices.
///
/// This trait is intended to be implemented on a unit struct, with the
/// `Request` and `Error` associated types specified as described.
pub trait RpcInterface {
    /// The request type.
    type Request: Serialize + DeserializeOwned;
    /// The error type.
    type Error: Serialize + DeserializeOwned + Fail;

    /// Returns a unique integer, identifying which API version this
    /// RPC interface is at.
    ///
    /// This is used to catch deserialization errors due to old/new API conflicts
    /// early.
    fn api_version() -> u8;
}

pub struct RpcRequestInProgress<'a, P> {
    listener: &'a mut RpcListener<P>,
    msg: nng::Message
}
impl<'a, P> RpcRequestInProgress<'a, P> where P: RpcInterface {
    pub fn decode(&self) -> Result<P::Request> {
        let data = self.msg.as_slice();
        if data.len() < 3 {
            Err(ProtoError::MalformedResponse("message way too short"))?
        }
        wire::check_header::<P>(data)?;
        let ret = rmp_serde::decode::from_slice(&data[2..])?;
        Ok(ret)
    }
    pub fn reply(mut self, resp: RpcResponse<P>) -> Result<()> {
        self.msg.clear();
        resp.to_wire(&mut self.msg)?;
        self.listener.socket.send(self.msg).map_err(|(_msg, e)| e)?;
        Ok(())
    }
}
pub struct RpcListener<P> {
    socket: Socket,
    _proto: PhantomData<P>
}
impl<P> RpcListener<P> where P: RpcInterface {
    pub fn new(listen_url: &str) -> Result<Self> { 
        let socket = Socket::new(Protocol::Rep0)?;
        socket.listen(listen_url)?;
        Ok(Self {
            socket,
            _proto: PhantomData
        })
    }
    pub fn recv<'a>(&'a mut self) -> Result<RpcRequestInProgress<'a, P>> {
        let msg = self.socket.recv()?;
        Ok(RpcRequestInProgress {
            listener: self,
            msg
        })
    }
}
pub struct RpcClient<P> {
    socket: Socket,
    _proto: PhantomData<P>
}
impl<P> RpcClient<P> where P: RpcInterface {
    pub fn new(dial_url: &str) -> Result<Self> { 
        let socket = Socket::new(Protocol::Req0)?;
        socket.dial(dial_url)?;
        Ok(Self {
            socket,
            _proto: PhantomData
        })
    }
    pub fn request<T>(&mut self, req: P::Request) -> Result<StdResult<T, P::Error>> where T: DeserializeOwned {
        let mut req_vec = vec![API_VERSION, P::api_version()];
        rmp_serde::encode::write(&mut req_vec, &req)?;
        self.socket.send(&req_vec).map_err(|(_msg, e)| e)?;
        let reply = self.socket.recv()?;
        let resp: RpcResponse<'_, P> = RpcResponse::from_wire(&reply)?;
        let ret = resp.into_result()?;
        Ok(ret)
    }
}
