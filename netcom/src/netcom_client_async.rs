use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::dto::ClientInfoRequestDto;
use crate::dto::ClientInfoResponseDto;
use crate::dto::DeviceDto;
use crate::dto::DeviceListRequestDto;
use crate::dto::DeviceListResponseDto;
use crate::dto::ReadResponseDto;
use crate::dto::UpgradeResponseDto;
use crate::dto::WrValueDto;
use crate::dto::WriteRequestDto;
use crate::dto::WriteResponseDto;
use crate::netcom::build_read_request;
use crate::netcom::parse_json;
use crate::netcom::NetcomError;
use crate::netcom::NetcomSync;
use crate::netcom::RdOp;
use crate::netcom::WrOp;
use crate::netstring::parse_netstring;
use crate::netstring::NetstringError;
use crate::netstring::ToNetstring;

pub struct NetcomClientAsync {
    hostname: String,
    port: u16,
    stream: Option<tokio::net::TcpStream>,
    auto_connect: bool,
    version: Option<String>,
}

impl NetcomClientAsync {
    pub fn new(hostname: &str, port: u16) -> Self {
        NetcomClientAsync {
            hostname: hostname.to_string(),
            port,
            stream: None,
            auto_connect: true,
            version: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), NetcomError> {
        let host = format!("{}:{}", self.hostname, self.port);
        let mut stream = tokio::net::TcpStream::connect(host)
            .await
            .map_err(|e| NetcomError::StreamError(e))?;

        stream
            .write_all(b"PROTO30\n")
            .await
            .map_err(|e| NetcomError::StreamError(e))?;

        self.stream = Some(stream);

        let json = match self.read_netstring().await {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        let response: UpgradeResponseDto = parse_json(&json)?;
        self.version = Some(response.version);
        Ok(())
    }

    async fn read_netstring(&mut self) -> Result<Vec<u8>, NetcomError> {
        // TODO: Handle timeout
        match &mut self.stream {
            Some(stream) => {
                let mut msg: Vec<u8> = Vec::new();
                let mut buf = [0; 128];

                loop {
                    match stream.read(&mut buf).await {
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

    async fn prepare(&mut self) -> Result<(), NetcomError> {
        if self.auto_connect && self.stream.is_none() {
            self.connect().await?;
        }
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn disconnect(&mut self) {
        self.stream = None;
    }

    async fn send_buf(&mut self, buf: &[u8]) -> Result<(), NetcomError> {
        self.prepare().await?;

        if let Some(s) = &mut self.stream {
            s.write(buf)
                .await
                .map_err(|e| NetcomError::StreamError(e))?;
            return Ok(());
        }

        Err(NetcomError::NotConnected)
    }

    async fn send_request<T>(&mut self, req: &T) -> Result<(), NetcomError>
    where
        T: Serialize,
    {
        match serde_json::to_string(req) {
            Ok(s) => self.send_buf(&s.to_netstring()).await,
            Err(e) => Err(NetcomError::JsonError(e)),
        }
    }

    async fn wait_for_response<T>(&mut self) -> Result<T, NetcomError>
    where
        T: DeserializeOwned,
    {
        match self.read_netstring().await {
            Ok(s) => parse_json(&s),
            Err(e) => Err(e),
        }
    }

    pub async fn get_device_list(&mut self) -> Result<Vec<DeviceDto>, NetcomError> {
        let req = DeviceListRequestDto {
            r: "device-list".to_string(),
        };

        self.send_request(&req).await?;

        match self.wait_for_response::<DeviceListResponseDto>().await {
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

    pub async fn push_client_info(&mut self, name: &str) -> Result<(), NetcomError> {
        let req = ClientInfoRequestDto {
            r: "client-info".to_string(),
            name: name.to_string(),
        };

        self.send_request(&req).await?;

        match self.wait_for_response::<ClientInfoResponseDto>().await {
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

    pub async fn read_parameters(
        &mut self,
        device: &str,
        parameters: Vec<RdOp>,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
        let req = build_read_request(device, parameters);
        self.send_request(&req).await?;

        match self.wait_for_response::<ReadResponseDto>().await {
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

    pub async fn write_parameters(
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

        self.send_request(&req).await?;

        match self.wait_for_response::<WriteResponseDto>().await {
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

    pub async fn read_struct<T: NetcomSync>(
        &mut self,
        device: &str,
        params: &mut T,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
        let rdops = params.to_rdops();
        match self.read_parameters(device, rdops).await {
            Ok(res) => {
                params.apply_result(&res);
                Ok(res)
            }
            Err(err) => Err(err),
        }
    }

    pub async fn write_struct<T: NetcomSync>(
        &mut self,
        device: &str,
        params: &T,
    ) -> Result<HashMap<String, Option<f64>>, NetcomError> {
        let wrops = params.to_wrops();
        match self.write_parameters(device, wrops).await {
            Ok(res) => Ok(res),
            Err(e) => Err(e),
        }
    }
}
impl Drop for NetcomClientAsync {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::netcom::DEFAULT_PORT;

    #[tokio::test]
    async fn should_connect() {
        let mut c = NetcomClientAsync::new("localhost", DEFAULT_PORT);
        match c.connect().await {
            Ok(()) => {}
            Err(e) => panic!("Failed with error: {:?}", e),
        }
    }
}
