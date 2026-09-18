use std::fs;
use std::process::Command;
use tracing::{info, warn};

/// Linux 桌面规范系统级权限管理器 (基于 PolicyKit / udev uaccess 规范)
pub struct SystemPermission;

impl SystemPermission {
    /// 诊断当前用户是否具备 Linux 内核输入设备 (/dev/input/event*) 读取权限
    pub fn has_input_permissions() -> bool {
        let Ok(entries) = fs::read_dir("/dev/input") else {
            return false;
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name.starts_with("event") {
                if let Ok(file) = fs::File::open(entry.path()) {
                    drop(file);
                    return true;
                }
            }
        }
        false
    }

    /// 规范调起 GNOME / Polkit 原生系统安全认证弹窗，一键配置 uaccess 规则
    /// 
    /// 1. 采用现代桌面推荐的 `TAG+="uaccess"` 规则写入 `/etc/udev/rules.d/99-vok-input.rules`
    /// 2. 即刻通过 `udevadm` 刷新规则并触发 input 匹配
    /// 3. 为当前登录用户附加直接读取 ACL（`setfacl`），实现零重启、零注销、即刻生效
    pub fn request_input_permissions() -> Result<bool, String> {
        let username = std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "lin".to_string());

        info!("【规范权限申请】通过 pkexec 调起系统原生 Polkit 授权对话框，申请配置 input udev 规则...");

        let cmd = format!(
            "echo 'KERNEL==\"event*\", SUBSYSTEM==\"input\", TAG+=\"uaccess\"' > /etc/udev/rules.d/99-vok-input.rules && udevadm control --reload && udevadm trigger --subsystem-match=input && (which setfacl >/dev/null 2>&1 && setfacl -m u:{username}:r /dev/input/event* || true)",
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
