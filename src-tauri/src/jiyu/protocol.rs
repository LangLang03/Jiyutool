use serde::{Deserialize, Serialize};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};

pub const DEFAULT_PORT: u16 = 4706;
pub const CHUNK_SIZE: usize = 32 * 1024;
pub const SCAN_TIMEOUT_MS: u64 = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub uuid: String,
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    pub last_seen: i64,
}

impl DeviceInfo {
    pub fn new(device_name: String, ip: String, port: u16) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            device_name,
            ip,
            port,
            last_seen: chrono::Utc::now().timestamp(),
        }
    }

    pub fn with_uuid(uuid: String, device_name: String, ip: String, port: u16) -> Self {
        Self {
            uuid,
            device_name,
            ip,
            port,
            last_seen: chrono::Utc::now().timestamp(),
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen = chrono::Utc::now().timestamp();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    #[serde(rename = "handshake")]
    Handshake {
        uuid: String,
        device_name: String,
        ip: String,
        port: u16,
    },
    #[serde(rename = "handshake_response")]
    HandshakeResponse {
        uuid: String,
        device_name: String,
        ip: String,
        port: u16,
    },
    #[serde(rename = "file_request")]
    FileRequest {
        transfer_id: String,
        file_name: String,
        file_size: u64,
        md5: String,
        is_folder: bool,
        total_files: u32,
        relative_path: Option<String>,
    },
    #[serde(rename = "file_accept")]
    FileAccept {
        transfer_id: String,
        save_path: String,
    },
    #[serde(rename = "file_reject")]
    FileReject {
        transfer_id: String,
        reason: String,
    },
    #[serde(rename = "file_data")]
    FileData {
        transfer_id: String,
        chunk_index: u64,
        total_chunks: u64,
        data: String,
    },
    #[serde(rename = "file_complete")]
    FileComplete {
        transfer_id: String,
    },
    #[serde(rename = "busy")]
    Busy {
        message: String,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

impl Message {
    pub fn handshake(uuid: &str, device_name: &str, ip: &str, port: u16) -> Self {
        Message::Handshake {
            uuid: uuid.to_string(),
            device_name: device_name.to_string(),
            ip: ip.to_string(),
            port,
        }
    }

    pub fn handshake_response(uuid: &str, device_name: &str, ip: &str, port: u16) -> Self {
        Message::HandshakeResponse {
            uuid: uuid.to_string(),
            device_name: device_name.to_string(),
            ip: ip.to_string(),
            port,
        }
    }

    pub fn file_request(
        transfer_id: &str,
        file_name: &str,
        file_size: u64,
        md5: &str,
        is_folder: bool,
        total_files: u32,
        relative_path: Option<&str>,
    ) -> Self {
        Message::FileRequest {
            transfer_id: transfer_id.to_string(),
            file_name: file_name.to_string(),
            file_size,
            md5: md5.to_string(),
            is_folder,
            total_files,
            relative_path: relative_path.map(|s| s.to_string()),
        }
    }

    pub fn file_accept(transfer_id: &str, save_path: &str) -> Self {
        Message::FileAccept {
            transfer_id: transfer_id.to_string(),
            save_path: save_path.to_string(),
        }
    }

    pub fn file_reject(transfer_id: &str, reason: &str) -> Self {
        Message::FileReject {
            transfer_id: transfer_id.to_string(),
            reason: reason.to_string(),
        }
    }

    pub fn file_data(transfer_id: &str, chunk_index: u64, total_chunks: u64, data: &[u8]) -> Self {
        Message::FileData {
            transfer_id: transfer_id.to_string(),
            chunk_index,
            total_chunks,
            data: general_purpose::STANDARD.encode(data),
        }
    }

    pub fn file_complete(transfer_id: &str) -> Self {
        Message::FileComplete {
            transfer_id: transfer_id.to_string(),
        }
    }

    pub fn busy(message: &str) -> Self {
        Message::Busy {
            message: message.to_string(),
        }
    }

    pub fn error(message: &str) -> Self {
        Message::Error {
            message: message.to_string(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub transfer_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub transferred_size: u64,
    pub current_file: String,
    pub current_file_index: u32,
    pub total_files: u32,
    pub speed: f64,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    Pending,
    WaitingAccept,
    Transferring,
    Completed,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingFileRequest {
    pub transfer_id: String,
    pub from_device: DeviceInfo,
    pub file_name: String,
    pub file_size: u64,
    pub md5: String,
    pub is_folder: bool,
    pub total_files: u32,
}

pub fn calculate_md5(data: &[u8]) -> String {
    let digest = md5::compute(data);
    format!("{:x}", digest)
}

pub fn calculate_file_md5(path: &std::path::Path) -> Result<String, String> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut hasher = md5::Context::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| format!("读取文件失败: {}", e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.consume(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.compute()))
}
