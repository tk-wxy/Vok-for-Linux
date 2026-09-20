use std::fs;
use std::process::Command;
use tracing::{info, warn};

/// Linux 桌面规范系统级权限管理器 (基于 PolicyKit / udev uaccess 规范)
pub struct SystemPermission;

impl SystemPermission {
    /// 诊断当前用户是否具备 Linux 内核所有键盘输入设备 (/dev/input/event*) 读取权限
    pub fn has_input_permissions() -> bool {
        // 1. 优先精细化检查 /proc/bus/input/devices 中所有活跃键盘设备
        if let Ok(content) = fs::read_to_string("/proc/bus/input/devices") {
            let mut found_any_kbd = false;
            for line in content.lines() {
                if line.starts_with("H: Handlers=") && line.contains("kbd") {
                    for token in line["H: Handlers=".len()..].split_whitespace() {
                        if token.starts_with("event") {
                            found_any_kbd = true;
                            let dev_path = format!("/dev/input/{}", token);
                            if fs::File::open(&dev_path).is_err() {
                                // 发现存在未授权的物理键盘设备 (如蓝牙 UHID 键盘)
                                return false;
                            }
                        }
                    }
                }
            }
            if found_any_kbd {
                return true;
            }
        }

        // 2. 兜底检查：若无法读取 /proc，则检查 /dev/input 下所有 event 设备
        let Ok(entries) = fs::read_dir("/dev/input") else {
            return false;
        };

        let mut count = 0;
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with("event") {
                count += 1;
                if fs::File::open(entry.path()).is_err() {
                    return false;
                }
            }
        }
        count > 0
    }

    /// 规范调起 GNOME / Polkit 原生系统安全认证弹窗，一键配置 uaccess 规则
    /// 
    /// 1. 采用早于 71-seat.rules 的 `70-vok-input.rules` 赋予 `TAG+="seat", ENV{ID_SEAT}="seat0"`
    /// 2. 同时添加 `TAG+="uaccess"`, `MODE="0660"`, `GROUP="input"` 规避蓝牙 UHID 缺失 seat 陷阱
    /// 3. 将当前用户加入 `input` 用户组作为硬件读取双重保障
    /// 4. 即刻通过 `udevadm` 刷新规则并为当前用户附加直接读写 ACL（`setfacl`）
    pub fn request_input_permissions() -> Result<bool, String> {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "lin".to_string());

        info!("【规范权限申请】通过 pkexec 调起系统原生 Polkit 授权对话框，申请配置 input udev 规则与权限组...");

        let cmd = format!(
            "rm -f /etc/udev/rules.d/99-vok-input.rules && echo 'KERNEL==\"event*\", SUBSYSTEM==\"input\", TAG+=\"uaccess\", TAG+=\"seat\", ENV{{ID_SEAT}}=\"seat0\", MODE=\"0660\", GROUP=\"input\"' > /etc/udev/rules.d/70-vok-input.rules && usermod -aG input {username} && udevadm control --reload && udevadm trigger --subsystem-match=input && (which setfacl >/dev/null 2>&1 && setfacl -m u:{username}:rw /dev/input/event* || true)",
            username = username
        );

        let output = Command::new("pkexec")
            .args(["bash", "-c", &cmd])
            .output()
            .map_err(|e| format!("无法启动 pkexec 系统授权组件: {}", e))?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            warn!("【权限申请取消或失败】pkexec 返回码: {:?}, 原因: {}", output.status.code(), err_msg);
            return Err(format!("系统授权未完成: {}", err_msg.trim()));
        }

        let granted = Self::has_input_permissions();
        if granted {
            info!("【权限就绪】成功获得 Linux 内核 /dev/input 硬件读取权限！");
        } else {
            warn!("【权限未完全生效】虽然授权脚本执行成功，但 /dev/input/event* 仍处于受限状态");
        }

        Ok(granted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_input_permissions_does_not_panic() {
        let _ = SystemPermission::has_input_permissions();
    }
}
