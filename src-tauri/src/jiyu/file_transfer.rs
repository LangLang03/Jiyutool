use crate::jiyu::protocol::{
    calculate_file_md5, DeviceInfo, IncomingFileRequest, Message, TransferProgress, TransferStatus,
    CHUNK_SIZE, DEFAULT_PORT,
};
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;
use walkdir::WalkDir;

pub type ProgressCallback = Box<dyn Fn(TransferProgress) + Send + Sync>;
pub type RequestCallback = Box<dyn Fn(IncomingFileRequest) + Send + Sync>;

pub struct FileTransferService {
    uuid: String,
    device_name: String,
    ip: String,
    port: u16,
    is_busy: Arc<Mutex<bool>>,
    current_transfer: Arc<Mutex<Option<String>>>,
    progress_callbacks: Arc<Mutex<Vec<ProgressCallback>>>,
    request_callbacks: Arc<Mutex<Vec<RequestCallback>>>,
    pending_requests: Arc<Mutex<HashMap<String, IncomingFileRequest>>>,
    accepted_transfers: Arc<Mutex<HashMap<String, String>>>,
    rejected_transfers: Arc<Mutex<HashMap<String, String>>>,
    running: Arc<Mutex<bool>>,
    app_handle: Arc<Mutex<Option<AppHandle>>>,
}

impl FileTransferService {
    pub fn new(device_name: String, ip: String, port: Option<u16>) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            device_name,
            ip,
            port: port.unwrap_or(DEFAULT_PORT),
            is_busy: Arc::new(Mutex::new(false)),
            current_transfer: Arc::new(Mutex::new(None)),
            progress_callbacks: Arc::new(Mutex::new(Vec::new())),
            request_callbacks: Arc::new(Mutex::new(Vec::new())),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            accepted_transfers: Arc::new(Mutex::new(HashMap::new())),
            rejected_transfers: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
            app_handle: Arc::new(Mutex::new(None)),
        }
    }
    
    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }
    
    fn emit_progress(&self, progress: &TransferProgress) {
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit("transfer-progress", progress);
        }
    }
    
    fn emit_request(&self, request: &IncomingFileRequest) {
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit("file-request", request);
        }
    }

    pub fn get_uuid(&self) -> &str {
        &self.uuid
    }

    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }

    pub fn get_ip(&self) -> &str {
        &self.ip
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn is_busy(&self) -> bool {
        *self.is_busy.lock().unwrap()
    }

    pub fn on_progress<F: Fn(TransferProgress) + Send + Sync + 'static>(&self, callback: F) {
        self.progress_callbacks.lock().unwrap().push(Box::new(callback));
    }

    pub fn on_request<F: Fn(IncomingFileRequest) + Send + Sync + 'static>(&self, callback: F) {
        self.request_callbacks.lock().unwrap().push(Box::new(callback));
    }

    pub fn start(&self) -> Result<(), String> {
        let mut running = self.running.lock().unwrap();
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).map_err(|e| format!("绑定端口失败: {}", e))?;

        let is_busy = Arc::clone(&self.is_busy);
        let current_transfer = Arc::clone(&self.current_transfer);
        let progress_callbacks = Arc::clone(&self.progress_callbacks);
        let request_callbacks = Arc::clone(&self.request_callbacks);
        let pending_requests = Arc::clone(&self.pending_requests);
        let accepted_transfers = Arc::clone(&self.accepted_transfers);
        let rejected_transfers = Arc::clone(&self.rejected_transfers);
        let running = Arc::clone(&self.running);
        let uuid = self.uuid.clone();
        let device_name = self.device_name.clone();
        let ip = self.ip.clone();
        let port = self.port;
        let app_handle = self.app_handle.lock().unwrap().clone();

        thread::spawn(move || {
            listener.set_nonblocking(true).ok();

            while *running.lock().unwrap() {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let is_busy = Arc::clone(&is_busy);
                        let current_transfer = Arc::clone(&current_transfer);
                        let progress_callbacks = Arc::clone(&progress_callbacks);
                        let request_callbacks = Arc::clone(&request_callbacks);
                        let pending_requests = Arc::clone(&pending_requests);
                        let accepted_transfers = Arc::clone(&accepted_transfers);
                        let rejected_transfers = Arc::clone(&rejected_transfers);
                        let uuid = uuid.clone();
                        let device_name = device_name.clone();
                        let ip = ip.clone();
                        let port = port;
                        let app_handle = app_handle.clone();

                        thread::spawn(move || {
                            handle_connection(
                                stream,
                                &uuid,
                                &device_name,
                                &ip,
                                port,
                                is_busy,
                                current_transfer,
                                progress_callbacks,
                                request_callbacks,
                                pending_requests,
                                accepted_transfers,
                                rejected_transfers,
                                app_handle,
                            );
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => {}
                }
            }
        });

        Ok(())
    }

    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    pub fn accept_file(&self, transfer_id: &str, save_path: &str) -> Result<(), String> {
        let mut accepted = self.accepted_transfers.lock().unwrap();
        accepted.insert(transfer_id.to_string(), save_path.to_string());
        Ok(())
    }

    pub fn reject_file(&self, transfer_id: &str, reason: &str) -> Result<(), String> {
        let mut rejected = self.rejected_transfers.lock().unwrap();
        rejected.insert(transfer_id.to_string(), reason.to_string());
        Ok(())
    }

    pub fn send_file(
        &self,
        target: &DeviceInfo,
        file_path: &str,
    ) -> Result<String, String> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err("文件不存在".to_string());
        }

        let transfer_id = Uuid::new_v4().to_string();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let file_size = fs::metadata(path)
            .map(|m| m.len())
            .map_err(|e| format!("获取文件大小失败: {}", e))?;

        let md5 = calculate_file_md5(path)?;

        let mut stream = TcpStream::connect((target.ip.as_str(), target.port))
            .map_err(|e| format!("连接目标失败: {}", e))?;

        let request = Message::file_request(&transfer_id, &file_name, file_size, &md5, false, 1, None);
        send_message(&mut stream, &request)?;

        let response = read_message(&mut stream)?;
        match response {
            Message::FileAccept { save_path, .. } => {
                self.set_busy(true);
                self.set_current_transfer(Some(&transfer_id));

                let result = self.send_file_data(&mut stream, &transfer_id, path, &save_path, &file_name);

                self.set_busy(false);
                self.clear_current_transfer();

                result?;
                Ok(transfer_id)
            }
            Message::FileReject { reason, .. } => Err(format!("对方拒绝接收: {}", reason)),
            Message::Busy { message } => Err(format!("对方忙碌: {}", message)),
            _ => Err("无效响应".to_string()),
        }
    }

    pub fn send_folder(
        &self,
        target: &DeviceInfo,
        folder_path: &str,
    ) -> Result<String, String> {
        let path = Path::new(folder_path);
        if !path.exists() || !path.is_dir() {
            return Err("文件夹不存在或不是目录".to_string());
        }

        let transfer_id = Uuid::new_v4().to_string();
        let folder_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let files: Vec<(String, u64, String)> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let file_path = e.path();
                let relative = file_path.strip_prefix(path).ok()?;
                let relative_str = relative.to_string_lossy().to_string();
                let size = fs::metadata(file_path).ok()?.len();
                let md5 = calculate_file_md5(file_path).ok()?;
                Some((file_path.to_string_lossy().to_string(), size, md5))
            })
            .collect();

        let total_files = files.len() as u32;
        let total_size: u64 = files.iter().map(|(_, s, _)| *s).sum();

        let mut stream = TcpStream::connect((target.ip.as_str(), target.port))
            .map_err(|e| format!("连接目标失败: {}", e))?;

        let request = Message::file_request(
            &transfer_id,
            &folder_name,
            total_size,
            "",
            true,
            total_files,
            None,
        );
        send_message(&mut stream, &request)?;

        let response = read_message(&mut stream)?;
        match response {
            Message::FileAccept { save_path, .. } => {
                self.set_busy(true);
                self.set_current_transfer(Some(&transfer_id));

                for (file_path, _, _) in &files {
                    let file_path = Path::new(file_path);
                    let relative = file_path.strip_prefix(path).unwrap();
                    let relative_str = relative.to_string_lossy().to_string();

                    let file_request = Message::file_request(
                        &transfer_id,
                        &file_path.file_name().unwrap().to_string_lossy(),
                        fs::metadata(file_path).unwrap().len(),
                        &calculate_file_md5(file_path)?,
                        true,
                        total_files,
                        Some(&relative_str),
                    );
                    send_message(&mut stream, &file_request)?;

                    let file_response = read_message(&mut stream)?;
                    match file_response {
                        Message::FileAccept { .. } => {
                            self.send_file_data(&mut stream, &transfer_id, file_path, &save_path, &relative_str)?;
                        }
                        Message::FileReject { reason, .. } => {
                            self.set_busy(false);
                            return Err(format!("对方拒绝接收: {}", reason));
                        }
                        _ => {}
                    }
                }

                let complete = Message::file_complete(&transfer_id);
                send_message(&mut stream, &complete)?;

                self.set_busy(false);
                self.clear_current_transfer();

                Ok(transfer_id)
            }
            Message::FileReject { reason, .. } => Err(format!("对方拒绝接收: {}", reason)),
            Message::Busy { message } => Err(format!("对方忙碌: {}", message)),
            _ => Err("无效响应".to_string()),
        }
    }

    fn send_file_data(
        &self,
        stream: &mut TcpStream,
        transfer_id: &str,
        file_path: &Path,
        _save_path: &str,
        file_name: &str,
    ) -> Result<(), String> {
        let mut file = File::open(file_path).map_err(|e| format!("打开文件失败: {}", e))?;

        let file_size = fs::metadata(file_path)
            .map(|m| m.len())
            .map_err(|e| format!("获取文件大小失败: {}", e))?;

        let total_chunks = (file_size as f64 / CHUNK_SIZE as f64).ceil() as u64;
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut transferred = 0u64;
        let mut chunk_index = 0u64;

        let start_time = std::time::Instant::now();

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| format!("读取文件失败: {}", e))?;

            if bytes_read == 0 {
                break;
            }

            let data_msg = Message::file_data(transfer_id, chunk_index, total_chunks, &buffer[..bytes_read]);
            send_message(stream, &data_msg)?;

            transferred += bytes_read as u64;
            chunk_index += 1;

            let elapsed = start_time.elapsed().as_secs_f64();
            let speed = if elapsed > 0.0 {
                transferred as f64 / elapsed / 1024.0 / 1024.0
            } else {
                0.0
            };

            let progress = TransferProgress {
                transfer_id: transfer_id.to_string(),
                file_name: file_name.to_string(),
                total_size: file_size,
                transferred_size: transferred,
                current_file: file_name.to_string(),
                current_file_index: 1,
                total_files: 1,
                speed,
                status: TransferStatus::Transferring,
            };

            self.emit_progress(&progress);

            for callback in self.progress_callbacks.lock().unwrap().iter() {
                callback(progress.clone());
            }
        }

        Ok(())
    }

    fn set_busy(&self, busy: bool) {
        *self.is_busy.lock().unwrap() = busy;
    }

    fn set_current_transfer(&self, transfer_id: Option<&str>) {
        *self.current_transfer.lock().unwrap() = transfer_id.map(|s| s.to_string());
    }

    fn clear_current_transfer(&self) {
        *self.current_transfer.lock().unwrap() = None;
    }
}

fn handle_connection(
    mut stream: TcpStream,
    local_uuid: &str,
    local_device_name: &str,
    local_ip: &str,
    local_port: u16,
    is_busy: Arc<Mutex<bool>>,
    current_transfer: Arc<Mutex<Option<String>>>,
    progress_callbacks: Arc<Mutex<Vec<ProgressCallback>>>,
    request_callbacks: Arc<Mutex<Vec<RequestCallback>>>,
    pending_requests: Arc<Mutex<HashMap<String, IncomingFileRequest>>>,
    accepted_transfers: Arc<Mutex<HashMap<String, String>>>,
    rejected_transfers: Arc<Mutex<HashMap<String, String>>>,
    app_handle: Option<AppHandle>,
) {
    let message = match read_message(&mut stream) {
        Ok(m) => m,
        Err(_) => return,
    };

    match message {
        Message::Handshake {
            uuid,
            device_name,
            ip,
            port,
        } => {
            let response = Message::handshake_response(local_uuid, local_device_name, local_ip, local_port);
            let _ = send_message(&mut stream, &response);
        }
        Message::FileRequest {
            transfer_id,
            file_name,
            file_size,
            md5,
            is_folder,
            total_files,
            relative_path,
        } => {
            if *is_busy.lock().unwrap() {
                let busy_msg = Message::busy("正在传输中，请稍后再试");
                let _ = send_message(&mut stream, &busy_msg);
                return;
            }

            let from_device = DeviceInfo::new("Unknown".to_string(), "Unknown".to_string(), 0);

            let request = IncomingFileRequest {
                transfer_id: transfer_id.clone(),
                from_device,
                file_name: file_name.clone(),
                file_size,
                md5,
                is_folder,
                total_files,
            };

            pending_requests.lock().unwrap().insert(transfer_id.clone(), request.clone());

            if let Some(handle) = &app_handle {
                let _ = handle.emit("file-request", &request);
            }

            for callback in request_callbacks.lock().unwrap().iter() {
                callback(request.clone());
            }

            loop {
                thread::sleep(Duration::from_millis(100));

                let save_path_opt = {
                    let accepted = accepted_transfers.lock().unwrap();
                    accepted.get(&transfer_id).cloned()
                };
                
                if let Some(save_path) = save_path_opt {
                    let accept_msg = Message::file_accept(&transfer_id, &save_path);
                    let _ = send_message(&mut stream, &accept_msg);

                    is_busy.lock().unwrap().clone_from(&true);
                    current_transfer.lock().unwrap().replace(transfer_id.clone());

                    let _ = receive_file_data(
                        &mut stream,
                        &transfer_id,
                        &save_path,
                        &file_name,
                        file_size,
                        progress_callbacks.clone(),
                        app_handle.clone(),
                    );

                    is_busy.lock().unwrap().clone_from(&false);
                    current_transfer.lock().unwrap().take();
                    return;
                }

                let rejected = rejected_transfers.lock().unwrap();
                if let Some(reason) = rejected.get(&transfer_id) {
                    let reject_msg = Message::file_reject(&transfer_id, reason);
                    let _ = send_message(&mut stream, &reject_msg);
                    return;
                }
            }
        }
        _ => {}
    }
}

fn receive_file_data(
    stream: &mut TcpStream,
    transfer_id: &str,
    save_path: &str,
    file_name: &str,
    total_size: u64,
    progress_callbacks: Arc<Mutex<Vec<ProgressCallback>>>,
    app_handle: Option<AppHandle>,
) -> Result<(), String> {
    let save_dir = Path::new(save_path);
    if !save_dir.exists() {
        fs::create_dir_all(save_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let file_path = save_dir.join(file_name);
    let mut file = File::create(&file_path).map_err(|e| format!("创建文件失败: {}", e))?;

    let mut transferred = 0u64;
    let start_time = std::time::Instant::now();

    loop {
        let message = read_message(stream)?;

        match message {
            Message::FileData {
                chunk_index,
                total_chunks,
                data,
                ..
            } => {
                let decoded = general_purpose::STANDARD.decode(&data).map_err(|e| format!("解码数据失败: {}", e))?;
                file.write_all(&decoded)
                    .map_err(|e| format!("写入文件失败: {}", e))?;

                transferred += decoded.len() as u64;

                let elapsed = start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 {
                    transferred as f64 / elapsed / 1024.0 / 1024.0
                } else {
                    0.0
                };

                let progress = TransferProgress {
                    transfer_id: transfer_id.to_string(),
                    file_name: file_name.to_string(),
                    total_size,
                    transferred_size: transferred,
                    current_file: file_name.to_string(),
                    current_file_index: 1,
                    total_files: 1,
                    speed,
                    status: TransferStatus::Transferring,
                };

                if let Some(handle) = &app_handle {
                    let _ = handle.emit("transfer-progress", &progress);
                }

                for callback in progress_callbacks.lock().unwrap().iter() {
                    callback(progress.clone());
                }

                if chunk_index >= total_chunks - 1 {
                    break;
                }
            }
            Message::FileComplete { .. } => break,
            _ => {}
        }
    }

    let progress = TransferProgress {
        transfer_id: transfer_id.to_string(),
        file_name: file_name.to_string(),
        total_size,
        transferred_size: total_size,
        current_file: file_name.to_string(),
        current_file_index: 1,
        total_files: 1,
        speed: 0.0,
        status: TransferStatus::Completed,
    };

    if let Some(handle) = &app_handle {
        let _ = handle.emit("transfer-progress", &progress);
    }

    for callback in progress_callbacks.lock().unwrap().iter() {
        callback(progress.clone());
    }

    Ok(())
}

fn send_message(stream: &mut TcpStream, message: &Message) -> Result<(), String> {
    let json = message.to_json().map_err(|e| format!("序列化失败: {}", e))?;
    let len = json.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| format!("发送长度失败: {}", e))?;
    stream
        .write_all(json.as_bytes())
        .map_err(|e| format!("发送消息失败: {}", e))?;
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> Result<Message, String> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("读取长度失败: {}", e))?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    stream
        .read_exact(&mut buf)
        .map_err(|e| format!("读取消息失败: {}", e))?;

    let json = String::from_utf8(buf).map_err(|e| format!("UTF-8解码失败: {}", e))?;
    Message::from_json(&json).map_err(|e| format!("解析消息失败: {}", e))
}
