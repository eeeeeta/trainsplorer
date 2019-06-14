use crate::{API_VERSION, StdResult, RpcInterface};
use crate::errors::{Result, ProtoError};
use std::marker::PhantomData;
use std::borrow::Cow;
use std::convert::TryFrom;
use serde::{Serialize};
use serde::de::DeserializeOwned;

pub fn check_header<P: RpcInterface>(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 2 {
        Err(ProtoError::MalformedResponse("header too short"))?
    }
    let proto_vers = bytes[0];
    if proto_vers != API_VERSION {
        Err(ProtoError::IncorrectProtoVersion(proto_vers))?
    }
    let app_vers = bytes[1];
    let expected_app_vers = P::api_version();
    if app_vers != expected_app_vers {
        Err(ProtoError::IncorrectAppVersion {
            message: app_vers,
            cur: expected_app_vers
        })?
    }
    Ok(())
}
/// A response obtained from an RPC call.
pub enum RpcResponse<'a, P> {
    Successful {
        data: Cow<'a, [u8]>,
        _proto: PhantomData<P>
    },
    Failure {
        error_data: Cow<'a, [u8]>,
        _proto: PhantomData<P>
    }
}
impl<P, T> TryFrom<StdResult<T, P::Error>> for RpcResponse<'static, P> where T: Serialize, P: RpcInterface {
    type Error = ProtoError;

    fn try_from(res: StdResult<T, P::Error>) -> Result<Self> {
        match res {
            Ok(v) => {
                let data = rmp_serde::encode::to_vec(&v)?;
                Ok(RpcResponse::Successful {
                    data: data.into(),
                    _proto: PhantomData
                })
            },
            Err(e) => {
                let data = rmp_serde::encode::to_vec(&e)?;
                Ok(RpcResponse::Failure {
                    error_data: data.into(),
                    _proto: PhantomData
                })
            },
        }
    }
}
impl<'a, P> AsRef<[u8]> for RpcResponse<'a, P> {
    fn as_ref(&self) -> &[u8] {
        use self::RpcResponse::*;

        match self {
            Successful { data, .. } => &*data,
            Failure { error_data, .. } => &*error_data
        }
    }
}
impl<'a, P> RpcResponse<'a, P> where P: RpcInterface {
    /// Wire format message type for the 'success' variant.
    pub const MESSAGE_SUCCESSFUL: u8 = 0;
    /// Wire format message type for the 'failure' variant.
    pub const MESSAGE_FAILURE: u8 = 1;

    /// Convert this `RpcResponse` into a `Result`, deserializing the success type according
    /// to the generic parameter `T`.
    pub fn into_result<T: DeserializeOwned>(&self) -> Result<::std::result::Result<T, P::Error>> {
        use self::RpcResponse::*;

        match self {
            Successful { data, .. } => {
                let ret: T = rmp_serde::decode::from_slice(&*data)?;
                Ok(Ok(ret))
            },
            Failure { error_data, .. } => {
                let ret: P::Error = rmp_serde::decode::from_slice(&*error_data)?;
                Ok(Err(ret))
            }
        }
    }
    /// Serialize this response to wire format.
    pub fn to_wire<W: std::io::Write>(&self, mut writer: W) -> Result<()> {
        writer.write_all(&[API_VERSION, P::api_version()])?;
        writer.write_all(self.as_ref())?;
        Ok(())
    }
    /// Deserialize a wire-format response.
    pub fn from_wire(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 3 {
            Err(ProtoError::MalformedResponse("message way too short"))?
        }
        check_header::<P>(bytes)?;
        let ty = bytes[2];
        match ty {
            Self::MESSAGE_SUCCESSFUL => {
                Ok(RpcResponse::Successful {
                    data: Cow::Borrowed(&bytes[2..]),
                    _proto: PhantomData
                })
            },
            Self::MESSAGE_FAILURE => {
                Ok(RpcResponse::Failure {
                    error_data: Cow::Borrowed(&bytes[2..]),
                    _proto: PhantomData
                })
            },
            _ => {
                Err(ProtoError::MalformedResponse("invalid type field"))
            }
        }
    }
}
