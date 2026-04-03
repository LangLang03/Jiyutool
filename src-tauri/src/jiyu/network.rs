use regex::Regex;
use std::net::{ToSocketAddrs, UdpSocket};
use std::thread;
use std::time::Duration;

fn validate_ip_segment(segment: &str) -> Result<u8, String> {
    let num: u8 = segment
        .parse()
        .map_err(|_| format!("IP段无效: {}", segment))?;
    Ok(num)
}

fn validate_ip_parts(parts: &[&str]) -> Result<(), String> {
    if parts.len() < 3 || parts.len() > 4 {
        return Err("IP格式无效: 需要3或4段".to_string());
    }
    
    for part in parts {
        if part.is_empty() {
            return Err("IP段不能为空".to_string());
        }
        if !part.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!("IP段包含非法字符: {}", part));
        }
        let num: u32 = part.parse().unwrap_or(256);
        if num > 255 {
            return Err(format!("IP段超出范围(0-255): {}", part));
        }
    }
    
    Ok(())
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
        validate_ip_parts(&ip_parts)?;
        if ip_parts.len() != 4 {
            return Err("IP范围格式需要完整的4段IP".to_string());
        }
        let start: u8 = ip_parts[3]
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
    } else if ip_str.contains("/24") {
        let base_ip = ip_str.replace("/24", "");
        let ip_parts: Vec<&str> = base_ip.split('.').collect();
        validate_ip_parts(&ip_parts)?;
        if ip_parts.len() != 4 {
            return Err("CIDR格式需要完整的4段IP".to_string());
        }
        for i in 1..=254 {
            ips.push(format!("{}.{}.{}.{}", ip_parts[0], ip_parts[1], ip_parts[2], i));
        }
    } else {
        let ip_parts: Vec<&str> = ip_str.split('.').collect();
        validate_ip_parts(&ip_parts)?;
        
        if ip_parts.len() == 3 {
            for i in 1..=255 {
                ips.push(format!("{}.{}.{}.{}", ip_parts[0], ip_parts[1], ip_parts[2], i));
            }
        } else if ip_parts.len() == 4 {
            ips.push(ip_str.to_string());
        }
    }

    if ips.is_empty() {
        return Err("未生成有效的IP地址".to_string());
    }

    Ok(ips)
}

pub fn send_udp_packet(ip: &str, port: u16, data: &[u8]) -> Result<(), String> {
    let addr = format!("{}:{}", ip, port);
    let socket_addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("解析地址失败: {}", e))?
        .next()
        .ok_or("解析地址失败")?;

    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("绑定套接字失败: {}", e))?;

    socket
        .send_to(data, socket_addr)
        .map_err(|e| format!("发送数据包失败: {}", e))?;

    Ok(())
}

pub fn send_packets_to_targets(
    ips: &[String],
    port: u16,
    packets: &[Vec<u8>],
    loop_count: u32,
    interval_secs: u64,
) -> Result<String, String> {
    for iteration in 0..loop_count {
        for ip in ips {
            for packet in packets {
                if let Err(e) = send_udp_packet(ip, port, packet) {
                    return Err(format!("发送到 {} 失败: {}", ip, e));
                }
            }
        }

        if loop_count > 1 && iteration < loop_count - 1 {
            thread::sleep(Duration::from_secs(interval_secs));
        }
    }

    Ok(format!("成功发送到 {} 个目标", ips.len()))
}

#[cfg(target_os = "windows")]
pub fn get_local_ip() -> Result<String, String> {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .map_err(|e| format!("获取本机IP失败: {}", e))
}

#[cfg(target_os = "windows")]
pub fn find_student_ports() -> Result<Vec<u16>, String> {
    use std::process::Command;

    let output = Command::new("tasklist")
        .args(&["/FI", "IMAGENAME eq StudentMain.exe"])
        .output()
        .map_err(|e| format!("运行tasklist失败: {}", e))?;

    let output_str = String::from_utf8_lossy(&output.stdout);

    let pid_re = Regex::new(r"StudentMain\.exe\s+(\d+)").unwrap();
    let _pid = pid_re
        .captures(&output_str)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .ok_or("未找到StudentMain.exe")?;

    let netstat_output = Command::new("netstat")
        .args(&["-ano"])
        .output()
        .map_err(|e| format!("运行netstat失败: {}", e))?;

    let netstat_str = String::from_utf8_lossy(&netstat_output.stdout);

    let local_ip = get_local_ip()?;
    let port_re = Regex::new(&format!(r"{}:(\d+).*LISTENING", local_ip)).unwrap();

    let ports: Vec<u16> = port_re
        .captures_iter(&netstat_str)
        .filter_map(|caps| caps.get(1)?.as_str().parse().ok())
        .collect();

    Ok(ports)
}

#[cfg(not(target_os = "windows"))]
pub fn get_local_ip() -> Result<String, String> {
    Err("此平台不支持".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn find_student_ports() -> Result<Vec<u16>, String> {
    Err("此平台不支持".to_string())
}
