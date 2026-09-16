use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crossbeam_channel::{unbounded, Receiver, Sender};
use evdev::{Device, EventType, Key};
use thiserror::Error;
use tracing::{debug, info, warn};

#[derive(Error, Debug)]
pub enum InputError {
    #[error("无法读取 /dev/input 目录: {0}")]
    Io(#[from] std::io::Error),
    #[error("未找到具备键盘特性的输入设备")]
    NoKeyboardFound,
    #[error("权限不足：当前用户无权读取 {0}，请确保用户已加入 input 用户组")]
    PermissionDenied(PathBuf),
}

/// 物理按键电平状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
    Repeat,
}

/// 物理按键事件
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyEvent {
    pub key_code: u16,
    pub state: KeyState,
    pub timestamp: Instant,
}

/// evdev 物理键态监听器
pub struct EvdevMonitor {
    running: Arc<AtomicBool>,
    event_rx: Receiver<HotkeyEvent>,
    worker_threads: Vec<JoinHandle<()>>,
}

impl EvdevMonitor {
    /// 自动扫描 `/dev/input` 并在后台子线程启动对所有键盘设备的监听
    /// 
    /// 监听特定目标物理按键（如 KEY_SPACE, KEY_LEFTALT 等）
    pub fn start(target_key: Key) -> Result<Self, InputError> {
        let devices = Self::discover_keyboards(target_key)?;
        if devices.is_empty() {
            return Err(InputError::NoKeyboardFound);
        }

        let running = Arc::new(AtomicBool::new(true));
        let (tx, rx) = unbounded();
        let mut worker_threads = Vec::new();

        for (path, mut device) in devices {
            let running_clone = running.clone();
            let tx_clone = tx.clone();
            let target_code = target_key.code();

            let handle = thread::Builder::new()
                .name(format!("vok-evdev-{}", path.file_name().unwrap_or_default().to_string_lossy()))
                .spawn(move || {
                    debug!("开始监听物理设备: {:?}", path);
                    while running_clone.load(Ordering::Relaxed) {
                        match device.fetch_events() {
                            Ok(events) => {
                                for ev in events {
                                    if ev.event_type() == EventType::KEY && ev.code() == target_code {
                                        let state = match ev.value() {
                                            1 => KeyState::Pressed,
                                            0 => KeyState::Released,
                                            2 => KeyState::Repeat,
                                            _ => continue,
                                        };

                                        let hotkey_event = HotkeyEvent {
                                            key_code: target_code,
                                            state,
                                            timestamp: Instant::now(),
                                        };

                                        if let Err(e) = tx_clone.send(hotkey_event) {
                                            warn!("发送按键事件失败 (UI已断开): {:?}", e);
                                            return;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                if running_clone.load(Ordering::Relaxed) {
                                    warn!("读取设备 {:?} 发生错误: {:?}", path, e);
                                }
                                break;
                            }
                        }
                    }
                    debug!("停止监听物理设备: {:?}", path);
                })
                .map_err(InputError::Io)?;

            worker_threads.push(handle);
        }

        Ok(Self {
            running,
            event_rx: rx,
            worker_threads,
        })
    }

    /// 获取事件接收通道
    pub fn event_receiver(&self) -> Receiver<HotkeyEvent> {
        self.event_rx.clone()
    }

    /// 停止监听并回收工作线程
    pub fn stop(self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.worker_threads {
            let _ = handle.join();
        }
    }

    /// 自动枚举系统中支持目标按键的物理输入设备
    pub fn discover_keyboards(target_key: Key) -> Result<Vec<(PathBuf, Device)>, InputError> {
        let input_dir = Path::new("/dev/input");
        let mut result = Vec::new();
        let mut had_permission_error = false;
        let mut last_perm_path = PathBuf::new();

        if !input_dir.exists() {
            return Err(InputError::NoKeyboardFound);
        }

        for entry in fs::read_dir(input_dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if file_name.starts_with("event") {
                match Device::open(&path) {
                    Ok(device) => {
                        // 检查设备是否支持按键且包含目标按键
                        if let Some(keys) = device.supported_keys() {
                            if keys.contains(target_key) {
                                info!("发现支持热键的输入设备: {} ({:?})", device.name().unwrap_or("未知"), path);
                                result.push((path, device));
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        had_permission_error = true;
                        last_perm_path = path;
                    }
                    Err(_) => {
                        // 忽略其他无法打开的非标准事件设备
                    }
                }
            }
        }

        if result.is_empty() && had_permission_error {
            return Err(InputError::PermissionDenied(last_perm_path));
        }

        Ok(result)
    }
}

/// 模拟输入监视器（供单元测试或权限不足时回退仿真）
#[derive(Clone)]
pub struct MockInputMonitor {
    tx: Sender<HotkeyEvent>,
    rx: Receiver<HotkeyEvent>,
}

impl Default for MockInputMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl MockInputMonitor {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx }
    }

    pub fn event_receiver(&self) -> Receiver<HotkeyEvent> {
        self.rx.clone()
    }

    pub fn simulate_press(&self, key_code: u16) {
        let _ = self.tx.send(HotkeyEvent {
            key_code,
            state: KeyState::Pressed,
            timestamp: Instant::now(),
        });
    }

    pub fn simulate_release(&self, key_code: u16) {
        let _ = self.tx.send(HotkeyEvent {
            key_code,
            state: KeyState::Released,
            timestamp: Instant::now(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_input_monitor_event_flow() {
        let monitor = MockInputMonitor::new();
        let rx = monitor.event_receiver();

        monitor.simulate_press(57); // Space
        let ev1 = rx.recv().expect("应当收到按下事件");
        assert_eq!(ev1.key_code, 57);
        assert_eq!(ev1.state, KeyState::Pressed);

        monitor.simulate_release(57);
        let ev2 = rx.recv().expect("应当收到释放事件");
        assert_eq!(ev2.key_code, 57);
        assert_eq!(ev2.state, KeyState::Released);
    }
}
