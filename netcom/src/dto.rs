use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct UpgradeResponseDto {
    pub version: String,
}

#[derive(Serialize)]
pub struct DeviceListRequestDto {
    pub r: String,
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
pub struct DeviceListResponseDto {
    #[serde(rename = "R")]
    pub r: String,

    pub devices: Vec<DeviceDto>,
}

#[derive(Serialize)]
pub struct ClientInfoRequestDto {
    pub r: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct ClientInfoResponseDto {
    #[serde(rename = "R")]
    pub r: String,
}

#[derive(Serialize)]
pub struct ReadRequestDto {
    pub r: String,
    pub device: String,
    pub p: HashMap<String, RdValueDto>,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum WrValueDto {
    Simple(f64),
    Detailed { v: f64, t: String },
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum RdValueDto {
    Default,
    Detailed { t: String },
}

#[derive(Serialize)]
pub struct WriteRequestDto {
    pub r: String,
    pub device: String,
    pub p: HashMap<String, WrValueDto>,
}

#[derive(Deserialize)]
pub struct ReadResponseDto {
    #[serde(rename = "R")]
    pub r: String,
    // pub device: Option<String>,
    pub result: HashMap<String, Option<f64>>,
    // pub errors: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
pub struct WriteResponseDto {
    #[serde(rename = "R")]
    pub r: String,
    // pub device: Option<String>,
    pub result: HashMap<String, Option<f64>>,
    // pub errors: Option<HashMap<String, String>>,
}
