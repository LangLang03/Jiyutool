mod jiyu;

use jiyu::commands::AppState;
use jiyu::discovery::DeviceDiscovery;
use jiyu::file_transfer::FileTransferService;
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let device_name = DeviceDiscovery::get_device_name();
    let local_ip = DeviceDiscovery::get_local_ip().unwrap_or_else(|_| "0.0.0.0".to_string());
    
    let file_transfer = Arc::new(FileTransferService::new(device_name.clone(), local_ip.clone(), None));
    let discovery = Arc::new(DeviceDiscovery::new(
        file_transfer.get_uuid().to_string(),
        device_name,
        local_ip,
        file_transfer.get_port(),
    ));
    
    let app_state = AppState {
        file_transfer: Arc::clone(&file_transfer),
        discovery: Arc::clone(&discovery),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            jiyu::commands::send_message,
            jiyu::commands::execute_command,
            jiyu::commands::reboot_target,
            jiyu::commands::shutdown_target,
            jiyu::commands::get_local_info,
            jiyu::commands::break_screen_control,
            jiyu::commands::continue_screen_control,
            jiyu::commands::parse_ip,
            jiyu::commands::kill_student_process,
            jiyu::commands::start_reverse_shell,
            jiyu::commands::init_file_transfer,
            jiyu::commands::scan_devices,
            jiyu::commands::get_device_list,
            jiyu::commands::send_file,
            jiyu::commands::send_folder,
            jiyu::commands::accept_file,
            jiyu::commands::reject_file,
            jiyu::commands::get_transfer_status,
            jiyu::commands::connect_device
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            file_transfer.set_app_handle(app_handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
