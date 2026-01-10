use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::str::{self, Utf8Error};

pub use netcom_macros::NetcomMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::netstring::{parse_netstring, NetstringError, ToNetstring};

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

pub struct NetcomClient {
    hostname: String,
    port: u16,
    stream: Option<TcpStream>,
    auto_connect: bool,
    version: Option<String>,
}

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

#[derive(Deserialize)]
struct UpgradeResponseDto {
    version: String,
}

#[derive(Serialize)]
struct DeviceListRequestDto {
    r: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceDto {
    pub id: u32,
    pub network: u32,
    pub name: String,
    pub description: String,

    #[serde(rename = "type")]
    pub device_type: String,
}

#[derive(Deserialize)]
struct DeviceListResponseDto {
    #[serde(rename = "R")]
    r: String,

    devices: Vec<DeviceDto>,
}

#[derive(Serialize)]
struct ClientInfoRequestDto {
    r: String,
    name: String,
}

#[derive(Deserialize)]
struct ClientInfoResponseDto {
    #[serde(rename = "R")]
    r: String,
}

#[derive(Serialize)]
struct ReadRequestDto {
    r: String,
    device: String,
    p: HashMap<String, RdValueDto>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum WrValueDto {
    Simple(f64),
    Detailed { v: f64, t: String },
}

#[derive(Serialize)]
#[serde(untagged)]
enum RdValueDto {
    Default,
    Detailed { t: String },
}

#[derive(Serialize)]
struct WriteRequestDto {
    r: String,
    device: String,
    p: HashMap<String, WrValueDto>,
}

#[derive(Deserialize)]
struct ReadResponseDto {
    #[serde(rename = "R")]
    r: String,
    // device: Option<String>,
    result: HashMap<String, Option<f64>>,
    // errors: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct WriteResponseDto {
    #[serde(rename = "R")]
    r: String,
    // device: Option<String>,
    result: HashMap<String, Option<f64>>,
    // errors: Option<HashMap<String, String>>,
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

fn parse_json<T>(data: &[u8]) -> Result<T, NetcomError>
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

impl NetcomClient {
    pub fn new(hostname: &str, port: u16) -> Self {
        NetcomClient {
            hostname: hostname.to_string(),
            port,
            stream: None,
            auto_connect: true,
            version: None,
        }
    }

    pub fn read_netstring(&mut self) -> Result<Vec<u8>, NetcomError> {
        // TODO: Handle timeout
        match &mut self.stream {
            Some(stream) => {
                let mut msg: Vec<u8> = Vec::new();
                let mut buf = [0; 128];

                loop {
                    match stream.read(&mut buf) {
                        Ok(n) => {
                            msg.extend_from_slice(&buf[..n]);
                            match parse_netstring(&msg) {
                                Ok(s) => return Ok(s.to_vec()),
                                Err(NetstringError::Incomplete) => {}
                                Err(e) => return Err(NetcomError::NetstringError(e)),
                            };
                        }
                        Err(e) => return Err(NetcomError::StreamError(e)),
                    }
                }
            }
            None => Err(NetcomError::NotConnected),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn connect(&mut self) -> Result<(), NetcomError> {
        let host = format!("{}:{}", self.hostname, self.port);
        let mut stream = TcpStream::connect(host).map_err(|e| NetcomError::StreamError(e))?;

        stream
            .write_all(b"PROTO30\n")
            .map_err(|e| NetcomError::StreamError(e))?;

        self.stream = Some(stream);

        let json = match self.read_netstring() {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        let response: UpgradeResponseDto = parse_json(&json)?;
        self.version = Some(response.version);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
    }

    pub fn prepare(&mut self) -> Result<(), NetcomError> {
        if self.auto_connect && self.stream.is_none() {
            self.connect()?;
        }
        Ok(())
    }

    pub fn send_buf(&mut self, buf: &[u8]) -> Result<(), NetcomError> {
        self.prepare()?;

        if let Some(s) = &mut self.stream {
            s.write(buf).map_err(|e| NetcomError::StreamError(e))?;
            return Ok(());
        }

        Err(NetcomError::NotConnected)
    }

    pub fn send_request<T>(&mut self, req: &T) -> Result<(), NetcomError>
    where
        T: Serialize,
    {
        match serde_json::to_string(req) {
            Ok(s) => self.send_buf(&s.to_netstring()),
            Err(e) => Err(NetcomError::JsonError(e)),
        }
    }

    pub fn wait_for_response<T>(&mut self) -> Result<T, NetcomError>
    where
        T: DeserializeOwned,
    {
        match self.read_netstring() {
            Ok(s) => parse_json(&s),
            Err(e) => Err(e),
        }
    }

    pub fn get_device_list(&mut self) -> Result<Vec<DeviceDto>, NetcomError> {
        let req = DeviceListRequestDto {
            r: "device-list".to_string(),
        };

        self.send_request(&req)?;

        match self.wait_for_response::<DeviceListResponseDto>() {
            Ok(r) => match r.r.as_str() {
                "device-list" => Ok(r.devices),
                _ => Err(NetcomError::ResponseError(format!(
                    "Expected response type device-list, got {:?}",
                    r.r
                ))),
            },

            Err(e) => Err(e),
        }
    }

    pub fn push_client_info(&mut self, name: &str) -> Result<(), NetcomError> {
        let req = ClientInfoRequestDto {
            r: "client-info".to_string(),
            name: name.to_string(),
        };

        self.send_request(&req)?;

        match self.wait_for_response::<ClientInfoResponseDto>() {
            Ok(r) => match r.r.as_str() {
                "client-info" => Ok(()),
                _ => Err(NetcomError::ResponseError(format!(
                    "Expected response type device-list, got {:?}",
                    r.r
                ))),
            },
            Err(e) => Err(e),
        }
    }

    pub fn read_parameters(
        &mut self,
        device: &str,
        parameters: Vec<RdOp>,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
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

        let req = ReadRequestDto {
            r: "read".to_string(),
            device: device.to_string(),
            p,
        };

        self.send_request(&req)?;

        match self.wait_for_response::<ReadResponseDto>() {
            Ok(r) => match r.r.as_str() {
                "read" => Ok(r.result),
                _ => Err(NetcomError::ResponseError(format!(
                    "Expected response type 'read', got {:?}",
                    r.r
                ))),
            },

            Err(e) => Err(e),
        }
    }

    pub fn write_parameters(
        &mut self,
        device: &str,
        parameters: Vec<WrOp>,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
        let mut p = HashMap::<String, WrValueDto>::new();

        for op in parameters {
            match op {
                WrOp::Default { p: pp, v } => p.insert(pp, WrValueDto::Simple(v)),
                WrOp::WithType { p: pp, t, v } => p.insert(pp, WrValueDto::Detailed { v, t }),
            };
        }

        let req = WriteRequestDto {
            r: "write".to_string(),
            device: device.to_string(),
            p,
        };

        self.send_request(&req)?;

        match self.wait_for_response::<WriteResponseDto>() {
            Ok(res) => match res.r.as_str() {
                "write" => Ok(res.result),
                _ => Err(NetcomError::ResponseError(format!(
                    "Expected response type 'write', got {:?}",
                    res.r
                ))),
            },

            Err(e) => Err(e),
        }
    }

    pub fn read_struct<T: NetcomSync>(
        &mut self,
        device: &str,
        params: &mut T,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
        let rdops = params.to_rdops();
        match self.read_parameters(device, rdops) {
            Ok(res) => {
                params.apply_result(&res);
                Ok(res)
            }
            Err(err) => Err(err),
        }
    }

    pub fn write_struct<T: NetcomSync>(
        &mut self,
        device: &str,
        params: &T,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
        let wrops = params.to_wrops();
        println!("WROPS IS {:?}", wrops);
        match self.write_parameters(device, wrops) {
            Ok(res) => Ok(res),
            Err(e) => Err(e),
        }
    }
}

impl Drop for NetcomClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn should_connect() {
        let mut c = NetcomClient::new("localhost", DEFAULT_PORT);
        match c.connect() {
            Ok(()) => {}
            Err(e) => panic!("Failed with error: {:?}", e),
        }
    }

    #[test]
    fn should_serialize_write_request_dto() {
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
