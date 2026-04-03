use crate::jiyu::protocol::{DeviceInfo, Message, DEFAULT_PORT, SCAN_TIMEOUT_MS};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct DeviceDiscovery {
    local_uuid: String,
    local_device_name: String,
    local_ip: String,
    local_port: u16,
    devices: Arc<Mutex<Vec<DeviceInfo>>>,
}

impl DeviceDiscovery {
    pub fn new(uuid: String, device_name: String, ip: String, port: u16) -> Self {
        Self {
            local_uuid: uuid,
            local_device_name: device_name,
            local_ip: ip,
            local_port: port,
            devices: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_devices(&self) -> Vec<DeviceInfo> {
        self.devices.lock().unwrap().clone()
    }

    pub fn clear_devices(&self) {
        self.devices.lock().unwrap().clear();
    }

    pub fn add_or_update_device(&self, device: DeviceInfo) {
        let mut devices = self.devices.lock().unwrap();
        if let Some(existing) = devices.iter_mut().find(|d| d.uuid == device.uuid) {
            existing.update_last_seen();
            existing.ip = device.ip;
            existing.port = device.port;
            existing.device_name = device.device_name;
        } else {
            devices.push(device);
        }
    }

    pub fn remove_offline_devices(&self, timeout_secs: i64) {
        let now = chrono::Utc::now().timestamp();
        let mut devices = self.devices.lock().unwrap();
        devices.retain(|d| now - d.last_seen < timeout_secs);
    }

    pub fn get_local_ip() -> Result<String, String> {
        local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .map_err(|e| format!("获取本机IP失败: {}", e))
    }

    pub fn get_device_name() -> String {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Unknown".to_string())
    }

    pub fn parse_ip_range(ip_str: &str) -> Result<Vec<String>, String> {
        let ip_str = ip_str.trim();

        if ip_str.is_empty() {
            return Err("IP地址不能为空".to_string());
        }

        if !ip_str.contains('.') {
            return Err("IP格式无效: 缺少点号分隔".to_string());
        }

        let mut ips = Vec::new();

        if ip_str.contains('-') {
            let parts: Vec<&str> = ip_str.split('-').collect();
            if parts.len() != 2 {
                return Err("IP范围格式无效".to_string());
            }
            let ip_parts: Vec<&str> = parts[0].split('.').collect();
            if ip_parts.len() != 4 {
                return Err("IP范围格式需要完整的4段IP".to_string());
            }
            let start: u8 = parts[0]
                .split('.')
                .last()
                .unwrap_or("0")
                .parse()
                .map_err(|_| "起始IP段无效")?;
            let end: u8 = parts[1]
                .parse()
                .map_err(|_| "结束IP段无效")?;

            if start > end {
                return Err("起始IP不能大于结束IP".to_string());
            }

            for i in start..=end.min(254) {
                ips.push(format!("{}.{}.{}.{}", ip_parts[0], ip_parts[1], ip_parts[2], i));
            }
        } else {
            let ip_parts: Vec<&str> = ip_str.split('.').collect();
            if ip_parts.len() == 3 {
                for i in 1..=254 {
                    ips.push(format!("{}.{}.{}.{}", ip_parts[0], ip_parts[1], ip_parts[2], i));
                }
            } else if ip_parts.len() == 4 {
                ips.push(ip_str.to_string());
            } else {
                return Err("IP格式无效".to_string());
            }
        }

        if ips.is_empty() {
            return Err("未生成有效的IP地址".to_string());
        }

        Ok(ips)
    }

    pub fn get_local_subnet(&self) -> Result<Vec<String>, String> {
        let parts: Vec<&str> = self.local_ip.split('.').collect();
        if parts.len() != 4 {
            return Err("本机IP格式无效".to_string());
        }
        let subnet = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
        Self::parse_ip_range(&subnet)
    }

    pub fn scan_subnet(&self, port: Option<u16>) -> Vec<DeviceInfo> {
        let port = port.unwrap_or(DEFAULT_PORT);
        let ips = match self.get_local_subnet() {
            Ok(ips) => ips,
            Err(_) => return Vec::new(),
        };

        self.scan_ips(&ips, port)
    }

    pub fn scan_ips(&self, ips: &[String], port: u16) -> Vec<DeviceInfo> {
        let found_devices: Arc<Mutex<Vec<DeviceInfo>>> = Arc::new(Mutex::new(Vec::new()));
        let local_uuid = self.local_uuid.clone();
        let local_device_name = self.local_device_name.clone();
        let local_ip = self.local_ip.clone();
        let local_port = self.local_port;

        let handles: Vec<_> = ips
            .iter()
            .filter(|ip| **ip != local_ip)
            .map(|ip| {
                let ip = ip.clone();
                let found_devices = Arc::clone(&found_devices);
                let local_uuid = local_uuid.clone();
                let local_device_name = local_device_name.clone();
                let local_ip = local_ip.clone();
                let local_port = local_port;

                thread::spawn(move || {
                    if let Some(device) = Self::try_handshake(
                        &ip,
                        port,
                        &local_uuid,
                        &local_device_name,
                        &local_ip,
                        local_port,
                    ) {
                        found_devices.lock().unwrap().push(device);
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        let devices = found_devices.lock().unwrap().clone();
        for device in &devices {
            self.add_or_update_device(device.clone());
        }

        devices
    }

    fn try_handshake(
        ip: &str,
        port: u16,
        local_uuid: &str,
        local_device_name: &str,
        local_ip: &str,
        local_port: u16,
    ) -> Option<DeviceInfo> {
        let addr = format!("{}:{}", ip, port);
        let socket_addrs = match addr.to_socket_addrs() {
            Ok(addrs) => addrs,
            Err(_) => return None,
        };

        let socket_addr = match socket_addrs.into_iter().next() {
            Some(addr) => addr,
            None => return None,
        };

        let mut stream = match TcpStream::connect_timeout(&socket_addr, Duration::from_millis(SCAN_TIMEOUT_MS)) {
            Ok(s) => s,
            Err(_) => return None,
        };

        use std::io::{Read, Write};

        let handshake = Message::handshake(local_uuid, local_device_name, local_ip, local_port);
        let json = match handshake.to_json() {
            Ok(j) => j,
            Err(_) => return None,
        };

        let len = json.len() as u32;
        if stream.write_all(&len.to_be_bytes()).is_err() {
            return None;
        }
        if stream.write_all(json.as_bytes()).is_err() {
            return None;
        }

        let mut len_buf = [0u8; 4];
        if stream.read_exact(&mut len_buf).is_err() {
            return None;
        }
        let response_len = u32::from_be_bytes(len_buf) as usize;

        let mut response_buf = vec![0u8; response_len];
        if stream.read_exact(&mut response_buf).is_err() {
            return None;
        }

        let response_str = match String::from_utf8(response_buf) {
            Ok(s) => s,
            Err(_) => return None,
        };

        let response = match Message::from_json(&response_str) {
            Ok(m) => m,
            Err(_) => return None,
        };

        match response {
            Message::HandshakeResponse {
                uuid,
                device_name,
                ip,
                port,
            } => Some(DeviceInfo::with_uuid(uuid, device_name, ip, port)),
            _ => None,
        }
    }

    pub fn get_device_by_uuid(&self, uuid: &str) -> Option<DeviceInfo> {
        self.devices
            .lock()
            .unwrap()
            .iter()
            .find(|d| d.uuid == uuid)
            .cloned()
    }

    pub fn connect_and_handshake(&self, ip: &str, port: u16) -> Option<DeviceInfo> {
        Self::try_handshake(
            ip,
            port,
            &self.local_uuid,
            &self.local_device_name,
            &self.local_ip,
            self.local_port,
        )
    }
}
