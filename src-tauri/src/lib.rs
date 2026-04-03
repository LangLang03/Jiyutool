mod jiyu;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            jiyu::commands::start_reverse_shell
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
