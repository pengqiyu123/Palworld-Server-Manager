use serde::{Deserialize, Serialize};
use tauri::command;

use crate::windows_process::hidden_command;

#[derive(Serialize, Deserialize, Clone)]
pub struct FirewallStatus {
    pub port_8211_open: bool,
    pub port_27015_open: bool,
    pub port_25575_open: bool,
    pub port_8212_open: bool,
}

// ==================== 防火墙 ====================

#[command]
pub async fn check_firewall_rules() -> Result<FirewallStatus, String> {
    let port_8211_open = check_firewall_port(8211, "UDP").unwrap_or(false);
    let port_27015_open = check_firewall_port(27015, "UDP").unwrap_or(false);
    let port_25575_open = check_firewall_port(25575, "TCP").unwrap_or(false);
    let port_8212_open = check_firewall_port(8212, "TCP").unwrap_or(false);

    Ok(FirewallStatus {
        port_8211_open,
        port_27015_open,
        port_25575_open,
        port_8212_open,
    })
}

fn check_firewall_port(port: u16, protocol: &str) -> Result<bool, String> {
    // 对字符串参数应用 shell_escape 防止注入（防御性编程）
    let protocol_escaped = shell_escape::escape(protocol.into());
    let output = hidden_command("powershell")
        .args([
            "-Command",
            &format!(
                "Get-NetFirewallRule -DisplayName '*Palworld*' -ErrorAction SilentlyContinue | Get-NetFirewallPortFilter | Where-Object {{ $_.LocalPort -eq {} -and $_.Protocol -eq {} }} | Select-Object -First 1",
                port, protocol_escaped
            ),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

#[command]
pub async fn add_firewall_rules() -> Result<String, String> {
    let rules = [
        (8211, "UDP", "Palworld Server Game"),
        (27015, "UDP", "Palworld Server Query"),
    ];

    for (port, protocol, name) in rules {
        // 对所有字符串参数应用 shell_escape 防止注入
        let name_escaped = shell_escape::escape(name.into());
        let protocol_escaped = shell_escape::escape(protocol.into());
        let output = hidden_command("powershell")
            .args([
                "-Command",
                &format!(
                    "New-NetFirewallRule -DisplayName {} -Direction Inbound -Protocol {} -LocalPort {} -Action Allow -ErrorAction SilentlyContinue",
                    name_escaped, protocol_escaped, port
                ),
            ])
            .output()
            .map_err(|e| e.to_string())?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("already exists") && !stderr.contains("已存在") {
                return Err(format!("添加规则失败: {}", stderr));
            }
        }
    }

    Ok("防火墙规则配置成功，请重启电脑使规则生效".to_string())
}
