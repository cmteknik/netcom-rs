use std::collections::HashMap;
use std::fmt;
use std::io::Write;
use std::str::Utf8Error;

pub use netcom_macros::NetcomMap;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::dto::{RdValueDto, ReadRequestDto};
use crate::netstring::NetstringError;

pub const DEFAULT_PORT: u16 = 7878;

#[derive(Debug)]
pub enum NetcomError {
    NotConnected,
    StreamError(std::io::Error),
    NetstringError(NetstringError),
    JsonError(serde_json::Error),
    Utf8Error(Utf8Error),
    ResponseError(String),
}

impl fmt::Display for NetcomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetcomError::NotConnected => write!(f, "{}", "Not Connected"),
            NetcomError::StreamError(err) => write!(f, "Stream error: {}", err),
            NetcomError::NetstringError(err) => {
                write!(f, "Netstring error: {}", err)
            }
            NetcomError::JsonError(err) => write!(f, "JSON error: {}", err),
            NetcomError::Utf8Error(err) => write!(f, "UTF8 error: {}", err),
            NetcomError::ResponseError(err) => write!(f, "Response error: {}", err),
        }
    }
}

impl std::error::Error for NetcomError {}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WrOp {
    Default { p: String, v: f64 },
    WithType { p: String, t: String, v: f64 },
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum RdOp {
    Default { p: String },
    WithType { p: String, t: String },
}

pub trait NetcomSync {
    fn to_wrops(&self) -> Vec<WrOp>;
    fn to_rdops(&self) -> Vec<RdOp>;
    fn apply_result(&mut self, result: &HashMap<String, Option<f64>>);
}

#[derive(Eq, Hash, PartialEq)]
pub enum Parameter {
    Address(String),
    AddressAndType(String, String),
}

pub(super) fn parse_json<T>(data: &[u8]) -> Result<T, NetcomError>
where
    T: DeserializeOwned,
{
    match std::str::from_utf8(data) {
        Ok(s) => match serde_json::from_str(&s) {
            Ok(r) => Ok(r),
            Err(e) => Err(NetcomError::JsonError(e)),
        },
        Err(e) => Err(NetcomError::Utf8Error(e)),
    }
}

pub(super) fn build_read_request(device: &str, parameters: Vec<RdOp>) -> ReadRequestDto {
    let mut p = HashMap::<String, RdValueDto>::new();

    for op in parameters {
        match op {
            RdOp::Default { p: pp } => {
                p.insert(pp, RdValueDto::Default);
            }
            RdOp::WithType { p: pp, t } => {
                p.insert(pp, RdValueDto::Detailed { t });
            }
        };
    }

    ReadRequestDto {
        r: "read".to_string(),
        device: device.to_string(),
        p,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use crate::dto::{WrValueDto, WriteRequestDto};
    use std::collections::HashMap;

    #[tokio::test]
    async fn should_serialize_write_request_dto() {
        let mut p = HashMap::<String, WrValueDto>::new();
        p.insert("p1".to_string(), WrValueDto::Simple(242.0));
        p.insert("p2".to_string(), WrValueDto::Simple(3.14159));
        p.insert(
            "p3".to_string(),
            WrValueDto::Detailed {
                v: 1.0,
                t: "i16".to_string(),
            },
        );
        let w = WriteRequestDto {
            r: "write".to_string(),
            device: "foo-device".to_string(),
            p,
        };

        let expected = json!({
            "r": "write",
            "device": "foo-device",
            "p": {
                "p1": 242.0,
                "p2": 3.14159,
                "p3": {
                    "v": 1.0,
                    "t": "i16"
                }
            }
        });

        match serde_json::to_string(&w) {
            Ok(s) => {
                let value: Value = serde_json::from_str(&s).unwrap();
                assert_eq!(value, expected);
            }
            Err(e) => panic!("JSON serialization failed: {:?}", e),
        }
    }
}
