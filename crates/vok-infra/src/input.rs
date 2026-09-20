use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crossbeam_channel::{unbounded, Receiver, Sender};
use evdev::{Device, EventType, Key};
use thiserror::Error;
use tracing::{debug, info, warn};

use vok_core::{HotkeyConfig, ModifierMask};

use crate::window::WorkAreaGeometry;

#[derive(Error, Debug)]
pub enum InputError {
    #[error("无法读取 /dev/input 目录: {0}")]
    Io(#[from] std::io::Error),
    #[error("未找到具备键盘特性的输入设备")]
    NoKeyboardFound,
    #[error("权限不足：当前用户无权读取 {0}，请确保用户已加入 input 用户组")]
    PermissionDenied(PathBuf),
    #[error("虚拟输入设备构建或注入失败: {0}")]
    UInputError(String),
    #[error("X11 注入失败: {0}")]
    X11Error(String),
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

/// 将领域键名（如 "Space", "F12", "a", "Return"）映射为 Linux 内核 evdev::Key
pub fn key_name_to_evdev_key(name: &str) -> Option<Key> {
    let lower = name.trim().to_ascii_lowercase();
    // F1 - F24
    if let Some(stripped) = lower.strip_prefix('f') {
        if let Ok(num) = stripped.parse::<u32>() {
            return match num {
                1 => Some(Key::KEY_F1),
                2 => Some(Key::KEY_F2),
                3 => Some(Key::KEY_F3),
                4 => Some(Key::KEY_F4),
                5 => Some(Key::KEY_F5),
                6 => Some(Key::KEY_F6),
                7 => Some(Key::KEY_F7),
                8 => Some(Key::KEY_F8),
                9 => Some(Key::KEY_F9),
                10 => Some(Key::KEY_F10),
                11 => Some(Key::KEY_F11),
                12 => Some(Key::KEY_F12),
                13 => Some(Key::KEY_F13),
                14 => Some(Key::KEY_F14),
                15 => Some(Key::KEY_F15),
                16 => Some(Key::KEY_F16),
                17 => Some(Key::KEY_F17),
                18 => Some(Key::KEY_F18),
                19 => Some(Key::KEY_F19),
                20 => Some(Key::KEY_F20),
                21 => Some(Key::KEY_F21),
                22 => Some(Key::KEY_F22),
                23 => Some(Key::KEY_F23),
                24 => Some(Key::KEY_F24),
                _ => None,
            };
        }
    }

    match lower.as_str() {
        "space" => Some(Key::KEY_SPACE),
        "return" | "enter" => Some(Key::KEY_ENTER),
        "escape" | "esc" => Some(Key::KEY_ESC),
        "backspace" => Some(Key::KEY_BACKSPACE),
        "tab" => Some(Key::KEY_TAB),
        "grave" | "`" | "~" | "asciitilde" => Some(Key::KEY_GRAVE),
        "minus" | "-" => Some(Key::KEY_MINUS),
        "equal" | "=" => Some(Key::KEY_EQUAL),
        "bracketleft" | "[" => Some(Key::KEY_LEFTBRACE),
        "bracketright" | "]" => Some(Key::KEY_RIGHTBRACE),
        "backslash" | "\\" => Some(Key::KEY_BACKSLASH),
        "semicolon" | ";" => Some(Key::KEY_SEMICOLON),
        "apostrophe" | "'" => Some(Key::KEY_APOSTROPHE),
        "comma" | "," => Some(Key::KEY_COMMA),
        "period" | "dot" | "." => Some(Key::KEY_DOT),
        "slash" | "/" => Some(Key::KEY_SLASH),
        "up" => Some(Key::KEY_UP),
        "down" => Some(Key::KEY_DOWN),
        "left" => Some(Key::KEY_LEFT),
        "right" => Some(Key::KEY_RIGHT),
        "pageup" | "prior" => Some(Key::KEY_PAGEUP),
        "pagedown" | "next" => Some(Key::KEY_PAGEDOWN),
        "home" => Some(Key::KEY_HOME),
        "end" => Some(Key::KEY_END),
        "insert" => Some(Key::KEY_INSERT),
        "delete" => Some(Key::KEY_DELETE),
        "a" => Some(Key::KEY_A),
        "b" => Some(Key::KEY_B),
        "c" => Some(Key::KEY_C),
        "d" => Some(Key::KEY_D),
        "e" => Some(Key::KEY_E),
        "f" => Some(Key::KEY_F),
        "g" => Some(Key::KEY_G),
        "h" => Some(Key::KEY_H),
        "i" => Some(Key::KEY_I),
        "j" => Some(Key::KEY_J),
        "k" => Some(Key::KEY_K),
        "l" => Some(Key::KEY_L),
        "m" => Some(Key::KEY_M),
        "n" => Some(Key::KEY_N),
        "o" => Some(Key::KEY_O),
        "p" => Some(Key::KEY_P),
        "q" => Some(Key::KEY_Q),
        "r" => Some(Key::KEY_R),
        "s" => Some(Key::KEY_S),
        "t" => Some(Key::KEY_T),
        "u" => Some(Key::KEY_U),
        "v" => Some(Key::KEY_V),
        "w" => Some(Key::KEY_W),
        "x" => Some(Key::KEY_X),
        "y" => Some(Key::KEY_Y),
        "z" => Some(Key::KEY_Z),
        "0" => Some(Key::KEY_0),
        "1" => Some(Key::KEY_1),
        "2" => Some(Key::KEY_2),
        "3" => Some(Key::KEY_3),
        "4" => Some(Key::KEY_4),
        "5" => Some(Key::KEY_5),
        "6" => Some(Key::KEY_6),
        "7" => Some(Key::KEY_7),
        "8" => Some(Key::KEY_8),
        "9" => Some(Key::KEY_9),
        _ => None,
    }
}

/// 跟踪全局物理键盘修饰键按下状态
#[derive(Debug)]
struct ModifierTracker {
    left_ctrl: AtomicBool,
    right_ctrl: AtomicBool,
    left_alt: AtomicBool,
    right_alt: AtomicBool,
    left_shift: AtomicBool,
    right_shift: AtomicBool,
    left_super: AtomicBool,
    right_super: AtomicBool,
}

impl ModifierTracker {
    fn new() -> Self {
        Self {
            left_ctrl: AtomicBool::new(false),
            right_ctrl: AtomicBool::new(false),
            left_alt: AtomicBool::new(false),
            right_alt: AtomicBool::new(false),
            left_shift: AtomicBool::new(false),
            right_shift: AtomicBool::new(false),
            left_super: AtomicBool::new(false),
            right_super: AtomicBool::new(false),
        }
    }

    /// 更新修饰键状态。如果是修饰键，返回 true；否则返回 false
    fn update_key(&self, code: u16, is_down: bool) -> bool {
        if code == Key::KEY_LEFTCTRL.code() {
            self.left_ctrl.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_RIGHTCTRL.code() {
            self.right_ctrl.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_LEFTALT.code() {
            self.left_alt.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_RIGHTALT.code() {
            self.right_alt.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_LEFTSHIFT.code() {
            self.left_shift.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_RIGHTSHIFT.code() {
            self.right_shift.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_LEFTMETA.code() {
            self.left_super.store(is_down, Ordering::SeqCst);
            true
        } else if code == Key::KEY_RIGHTMETA.code() {
            self.right_super.store(is_down, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    fn current_mask(&self) -> ModifierMask {
        ModifierMask {
            ctrl: self.left_ctrl.load(Ordering::SeqCst) || self.right_ctrl.load(Ordering::SeqCst),
            alt: self.left_alt.load(Ordering::SeqCst) || self.right_alt.load(Ordering::SeqCst),
            shift: self.left_shift.load(Ordering::SeqCst) || self.right_shift.load(Ordering::SeqCst),
            super_key: self.left_super.load(Ordering::SeqCst) || self.right_super.load(Ordering::SeqCst),
        }
    }

    fn matches(&self, target_mods: &ModifierMask) -> bool {
        let cur = self.current_mask();
        cur.ctrl == target_mods.ctrl
            && cur.alt == target_mods.alt
            && cur.shift == target_mods.shift
            && cur.super_key == target_mods.super_key
    }
}

/// evdev 目标快捷键配置
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvdevHotkeyTarget {
    pub target_key: Key,
    pub modifiers: ModifierMask,
}

impl EvdevHotkeyTarget {
    pub fn from_config(config: &HotkeyConfig) -> Option<Self> {
        let target_key = key_name_to_evdev_key(&config.key_name)?;
        Some(Self {
            target_key,
            modifiers: config.modifiers,
        })
    }
}

/// evdev 物理键态监听器
pub struct EvdevMonitor {
    running: Arc<AtomicBool>,
    target: Arc<RwLock<Option<EvdevHotkeyTarget>>>,
    #[allow(dead_code)]
    modifiers: Arc<ModifierTracker>,
    is_hotkey_active: Arc<AtomicBool>,
    event_rx: Receiver<HotkeyEvent>,
    event_tx: Sender<HotkeyEvent>,
    monitored_paths: Arc<Mutex<HashSet<PathBuf>>>,
    scanner_handle: Option<JoinHandle<()>>,
}

impl EvdevMonitor {
    /// 自动扫描 `/dev/input` 并在后台启动对所有键盘设备的物理电平监听及动态热插拔自检
    pub fn start(initial_config: Option<&HotkeyConfig>) -> Result<Self, InputError> {
        let target = initial_config.and_then(EvdevHotkeyTarget::from_config);
        let target_lock = Arc::new(RwLock::new(target));
        let modifiers = Arc::new(ModifierTracker::new());
        let is_hotkey_active = Arc::new(AtomicBool::new(false));

        let running = Arc::new(AtomicBool::new(true));
        let (tx, rx) = unbounded();
        let monitored_paths = Arc::new(Mutex::new(HashSet::new()));

        // 1. 初次枚举并尝试挂载当前所有可读键盘设备
        Self::scan_and_attach(
            &monitored_paths,
            &running,
            &tx,
            &target_lock,
            &modifiers,
            &is_hotkey_active,
        );

        // 2. 启动后台动态热插拔与权限复活扫描线程 (每 2 秒自检)
        let running_scan = running.clone();
        let monitored_paths_scan = monitored_paths.clone();
        let tx_scan = tx.clone();
        let target_scan = target_lock.clone();
        let modifiers_scan = modifiers.clone();
        let active_scan = is_hotkey_active.clone();

        let scanner_handle = thread::Builder::new()
            .name("vok-evdev-scanner".to_string())
            .spawn(move || {
                while running_scan.load(Ordering::Relaxed) {
                    thread::sleep(std::time::Duration::from_secs(2));
                    if !running_scan.load(Ordering::Relaxed) {
                        break;
                    }
                    Self::scan_and_attach(
                        &monitored_paths_scan,
                        &running_scan,
                        &tx_scan,
                        &target_scan,
                        &modifiers_scan,
                        &active_scan,
                    );
                }
            })
            .ok();

        Ok(Self {
            running,
            target: target_lock,
            modifiers,
            is_hotkey_active,
            event_rx: rx,
            event_tx: tx,
            monitored_paths,
            scanner_handle,
        })
    }

    /// 扫描 /dev/input 并对新接入/新获权的键盘设备挂载监听线程
    fn scan_and_attach(
        monitored_paths: &Arc<Mutex<HashSet<PathBuf>>>,
        running: &Arc<AtomicBool>,
        tx: &Sender<HotkeyEvent>,
        target: &Arc<RwLock<Option<EvdevHotkeyTarget>>>,
        modifiers: &Arc<ModifierTracker>,
        is_hotkey_active: &Arc<AtomicBool>,
    ) {
        let input_dir = Path::new("/dev/input");
        let Ok(entries) = fs::read_dir(input_dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.starts_with("event") {
                continue;
            }

            // 检查是否已经在监听中
            {
                let guard = monitored_paths.lock().unwrap();
                if guard.contains(&path) {
                    continue;
                }
            }

            // 尝试打开设备
            let Ok(mut device) = Device::open(&path) else {
                continue;
            };

            // 检查是否具备键盘按键
            let Some(keys) = device.supported_keys() else {
                continue;
            };

            if !keys.contains(Key::KEY_SPACE)
                && !keys.contains(Key::KEY_ENTER)
                && !keys.contains(Key::KEY_A)
                && !keys.contains(Key::KEY_F12)
                && !keys.contains(Key::KEY_ESC)
            {
                continue;
            }

            // 成功打开且为键盘设备，加入已监听集合
            {
                let mut guard = monitored_paths.lock().unwrap();
                guard.insert(path.clone());
            }

            info!(
                "【evdev动态挂载】成功接入键盘输入设备: {} ({:?})",
                device.name().unwrap_or("未知"),
                path
            );

            let running_clone = running.clone();
            let tx_clone = tx.clone();
            let target_clone = target.clone();
            let modifiers_clone = modifiers.clone();
            let active_clone = is_hotkey_active.clone();
            let monitored_paths_clone = monitored_paths.clone();
            let path_clone = path.clone();

            let _ = thread::Builder::new()
                .name(format!(
                    "vok-evdev-{}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ))
                .spawn(move || {
                    debug!("开始监听物理键盘设备: {:?}", path_clone);
                    while running_clone.load(Ordering::Relaxed) {
                        match device.fetch_events() {
                            Ok(events) => {
                                for ev in events {
                                    if ev.event_type() != EventType::KEY {
                                        continue;
                                    }

                                    let code = ev.code();
                                    let val = ev.value();
                                    // 1: press, 0: release
                                    if val != 0 && val != 1 {
                                        continue;
                                    }
                                    let is_down = val == 1;

                                    let is_modifier = modifiers_clone.update_key(code, is_down);
                                    let current_target = target_clone.read().unwrap().clone();
                                    let Some(target) = current_target else {
                                        continue;
                                    };

                                    let target_code = target.target_key.code();

                                    if is_modifier || code == target_code {
                                        info!(
                                            "【evdev硬件事件】设备 {:?}: code={}, val={}, is_mod={}, matches_target={}",
                                            path_clone.file_name().unwrap_or_default(),
                                            code,
                                            val,
                                            is_modifier,
                                            modifiers_clone.matches(&target.modifiers)
                                        );
                                    }

                                    if is_modifier {
                                        if !is_down && active_clone.load(Ordering::SeqCst) {
                                            if !modifiers_clone.matches(&target.modifiers) {
                                                if active_clone.swap(false, Ordering::SeqCst) {
                                                    info!("【evdev解除激活】修饰键释放导致热键注销: target_code={}", target_code);
                                                    let hotkey_event = HotkeyEvent {
                                                        key_code: target_code,
                                                        state: KeyState::Released,
                                                        timestamp: Instant::now(),
                                                    };
                                                    if let Err(e) = tx_clone.send(hotkey_event) {
                                                        warn!("发送按键事件失败 (UI已断开): {:?}", e);
                                                        return;
                                                    }
                                                }
                                            }
                                        }
                                    } else if code == target_code {
                                        if is_down {
                                            if modifiers_clone.matches(&target.modifiers) {
                                                if !active_clone.swap(true, Ordering::SeqCst) {
                                                    info!("【evdev触发激活】Pressed: target_code={}", target_code);
                                                    let hotkey_event = HotkeyEvent {
                                                        key_code: target_code,
                                                        state: KeyState::Pressed,
                                                        timestamp: Instant::now(),
                                                    };
                                                    if let Err(e) = tx_clone.send(hotkey_event) {
                                                        warn!("发送按键事件失败 (UI已断开): {:?}", e);
                                                        return;
                                                    }
                                                }
                                            }
                                        } else {
                                            if active_clone.swap(false, Ordering::SeqCst) {
                                                info!("【evdev触发释放】Released: target_code={}", target_code);
                                                let hotkey_event = HotkeyEvent {
                                                    key_code: target_code,
                                                    state: KeyState::Released,
                                                    timestamp: Instant::now(),
                                                };
                                                if let Err(e) = tx_clone.send(hotkey_event) {
                                                    warn!("发送按键事件失败 (UI已断开): {:?}", e);
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                if running_clone.load(Ordering::Relaxed) {
                                    warn!("读取键盘设备 {:?} 发生错误/拔出: {:?}", path_clone, e);
                                }
                                break;
                            }
                        }
                    }
                    let mut guard = monitored_paths_clone.lock().unwrap();
                    guard.remove(&path_clone);
                    debug!("停止监听物理设备并从集合注销: {:?}", path_clone);
                });
        }
    }

    /// 立即强制执行一次新设备挂载扫描 (供外部授权成功后回调)
    pub fn scan_now(&self) {
        Self::scan_and_attach(
            &self.monitored_paths,
            &self.running,
            &self.event_tx,
            &self.target,
            &self.modifiers,
            &self.is_hotkey_active,
        );
    }

    /// 查询当前是否存在处于活跃监听中的键盘物理设备
    pub fn has_active_devices(&self) -> bool {
        !self.monitored_paths.lock().unwrap().is_empty()
    }

    /// 查询当前目标按键是否处于物理按下激活状态
    pub fn is_hotkey_active(&self) -> bool {
        self.is_hotkey_active.load(Ordering::SeqCst)
    }

    /// 快捷使用单键启动监听 (兼容辅助)
    pub fn start_with_key(target_key: Key) -> Result<Self, InputError> {
        let monitor = Self::start(None)?;
        let target = EvdevHotkeyTarget {
            target_key,
            modifiers: ModifierMask::EMPTY,
        };
        *monitor.target.write().unwrap() = Some(target);
        Ok(monitor)
    }

    /// 动态更新目标监听快捷键
    pub fn update_target_hotkey(&self, config: &HotkeyConfig) {
        let new_target = EvdevHotkeyTarget::from_config(config);
        info!(
            "【EvdevMonitor】热重载目标按键: config={:?}, 解析evdev目标: {:?}",
            config.to_gsettings_string(),
            new_target
        );
        *self.target.write().unwrap() = new_target;
        self.is_hotkey_active.store(false, Ordering::SeqCst);
    }

    /// 获取事件接收通道
    pub fn event_receiver(&self) -> Receiver<HotkeyEvent> {
        self.event_rx.clone()
    }

    /// 停止监听并回收工作线程
    pub fn stop(self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.scanner_handle {
            let _ = handle.join();
        }
    }

    /// 自动枚举系统中支持键盘输入的物理设备
    pub fn discover_keyboards() -> Result<Vec<(PathBuf, Device)>, InputError> {
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
                        if let Some(keys) = device.supported_keys() {
                            if keys.contains(Key::KEY_SPACE)
                                || keys.contains(Key::KEY_ENTER)
                                || keys.contains(Key::KEY_A)
                                || keys.contains(Key::KEY_F12)
                                || keys.contains(Key::KEY_ESC)
                            {
                                info!(
                                    "发现键盘输入设备: {} ({:?})",
                                    device.name().unwrap_or("未知"),
                                    path
                                );
                                result.push((path, device));
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        had_permission_error = true;
                        last_perm_path = path;
                    }
                    Err(_) => {}
                }
            }
        }

        if result.is_empty() && had_permission_error {
            return Err(InputError::PermissionDenied(last_perm_path));
        }

        Ok(result)
    }
}

/// 基于 Linux 内核 evdev 的物理鼠标硬件级外置点击监听器
/// 
/// 直接从 `/dev/input/event*` 读取物理鼠标按键 `BTN_LEFT`，脱离任何 Wayland/Xwayland 合成器事件吞噬限制。
/// 只要用户物理按下鼠标左键，即刻比对当前光标物理绝对坐标：
/// - 若在工作区外部（如 Ubuntu Dock 纯黑空白缝隙、顶栏、多屏盲区）：触发自动隐藏；
/// - 若在工作区内部：静默放行，由 GTK 原生组件正常处理；
/// - 鼠标任意移动与晃动：产生 0 个按键事件，100% 免疫误退出！
pub struct EvdevMouseWatcher {
    running: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    workarea: Arc<RwLock<WorkAreaGeometry>>,
    vok_xid: Arc<AtomicU32>,
    #[allow(dead_code)]
    worker_threads: Vec<JoinHandle<()>>,
}

impl EvdevMouseWatcher {
    /// 启动后台硬件鼠标监听线程
    pub fn start<F: Fn() + Send + Sync + 'static>(
        initial_workarea: WorkAreaGeometry,
        initial_vok_xid: u32,
        on_outside_click: F,
    ) -> Result<Self, InputError> {
        let devices = Self::discover_mice()?;
        if devices.is_empty() {
            return Err(InputError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "未找到具备鼠标按键特性的输入设备",
            )));
        }

        let running = Arc::new(AtomicBool::new(true));
        let enabled = Arc::new(AtomicBool::new(false));
        let workarea = Arc::new(RwLock::new(initial_workarea));
        let vok_xid = Arc::new(AtomicU32::new(initial_vok_xid));
        let on_click = Arc::new(on_outside_click);
        let mut worker_threads = Vec::new();

        for (path, mut device) in devices {
            let running_clone = running.clone();
            let enabled_clone = enabled.clone();
            let workarea_clone = workarea.clone();
            let vok_xid_clone = vok_xid.clone();
            let on_click_clone = on_click.clone();

            let handle = thread::Builder::new()
                .name(format!("vok-mouse-{}", path.file_name().unwrap_or_default().to_string_lossy()))
                .spawn(move || {
                    debug!("开始监听物理鼠标设备: {:?}", path);
                    let mut touch_down_instant: Option<Instant> = None;

                    while running_clone.load(Ordering::Relaxed) {
                        match device.fetch_events() {
                            Ok(events) => {
                                for ev in events {
                                    // 1. 触控板轻击 (Tap) 状态机追踪 (BTN_TOUCH 330: 按下至抬起 < 250ms 构成轻击)
                                    let is_tap = if ev.event_type() == EventType::KEY && ev.code() == Key::BTN_TOUCH.code() {
                                        if ev.value() == 1 {
                                            touch_down_instant = Some(Instant::now());
                                            false
                                        } else if ev.value() == 0 {
                                            if let Some(down) = touch_down_instant.take() {
                                                down.elapsed().as_millis() < 250
                                            } else {
                                                false
                                            }
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };

                                    // 2. 物理鼠标按键 (左键、右键、中键、侧键) 物理按下 (value == 1)
                                    let is_button_press = ev.event_type() == EventType::KEY
                                        && (ev.code() == Key::BTN_LEFT.code()
                                            || ev.code() == Key::BTN_RIGHT.code()
                                            || ev.code() == Key::BTN_MIDDLE.code()
                                            || ev.code() == Key::BTN_SIDE.code()
                                            || ev.code() == Key::BTN_EXTRA.code())
                                        && ev.value() == 1;

                                    if is_button_press || is_tap {
                                        let current_xid = vok_xid_clone.load(Ordering::Relaxed);
                                        let current_wa = *workarea_clone.read().unwrap();

                                        if let Some((is_outside, x, y, child)) = crate::WindowAdapter::check_pointer_against_vok(current_wa, current_xid) {
                                            info!(
                                                "【内核硬件物理输入】设备 {:?} 捕获到{} (code={}, val={}) | 光标: ({}, {}), child=0x{:x}, vok_xid=0x{:x} | is_outside={}",
                                                path.file_name().unwrap_or_default(),
                                                if is_tap { "触控板轻触Tap" } else { "按键点击Click" },
                                                ev.code(), ev.value(),
                                                x, y, child, current_xid, is_outside
                                            );

                                            if !enabled_clone.load(Ordering::Relaxed) {
                                                debug!("【硬件点击跳过】当前监听处于非激活状态 (enabled=false)");
                                                continue;
                                            }

                                            if is_outside {
                                                info!(
                                                    "【内核硬件级外置点击感知】确认在工作区外部点击 (x={}, y={}, child=0x{:x})，立即触发自动隐藏",
                                                    x, y, child
                                                );
                                                // 自动临时禁用避免短时重复抖动
                                                enabled_clone.store(false, Ordering::Relaxed);
                                                on_click_clone();
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                if running_clone.load(Ordering::Relaxed) {
                                    warn!("读取鼠标设备 {:?} 错误: {:?}", path, e);
                                }
                                break;
                            }
                        }
                    }
                    debug!("停止监听物理鼠标设备: {:?}", path);
                })
                .map_err(InputError::Io)?;

            worker_threads.push(handle);
        }

        Ok(Self {
            running,
            enabled,
            workarea,
            vok_xid,
            worker_threads,
        })
    }

    /// 自动枚举系统中支持鼠标按键的物理输入设备
    pub fn discover_mice() -> Result<Vec<(PathBuf, Device)>, InputError> {
        let input_dir = Path::new("/dev/input");
        let mut result = Vec::new();
        let mut had_permission_error = false;
        let mut last_perm_path = PathBuf::new();

        if !input_dir.exists() {
            return Err(InputError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "/dev/input 目录不存在",
            )));
        }

        for entry in fs::read_dir(input_dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            if file_name.starts_with("event") {
                match Device::open(&path) {
                    Ok(device) => {
                        if let Some(keys) = device.supported_keys() {
                            if keys.contains(Key::BTN_LEFT) || keys.contains(Key::BTN_RIGHT) {
                                info!("发现支持鼠标按键的物理设备: {} ({:?})", device.name().unwrap_or("未知"), path);
                                result.push((path, device));
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        had_permission_error = true;
                        last_perm_path = path;
                    }
                    Err(_) => {}
                }
            }
        }

        if result.is_empty() && had_permission_error {
            return Err(InputError::PermissionDenied(last_perm_path));
        }

        Ok(result)
    }

    /// 更新当前生效的工作区几何
    pub fn update_workarea(&self, geom: WorkAreaGeometry) {
        if let Ok(mut lock) = self.workarea.write() {
            *lock = geom;
        }
    }

    /// 更新当前生效的 Vok 窗口 XID
    pub fn update_vok_xid(&self, xid: u32) {
        self.vok_xid.store(xid, Ordering::Relaxed);
    }

    /// 启用/禁用点击检测 (当 Vok 呈现时启用，隐藏或处于代理模式时禁用)
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

impl Drop for EvdevMouseWatcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
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

use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
use evdev::AttributeSet;
use evdev::InputEvent;
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{KEY_PRESS_EVENT, KEY_RELEASE_EVENT};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

/// 虚拟按键注入器 (双引擎策略：优先 Linux 内核 uinput；无权限时自动回退 X11 XTEST)
pub struct InputInjector {
    uinput_device: Option<VirtualDevice>,
    has_uinput_permission: bool,
    x11_conn: Option<(RustConnection, usize)>,
}

impl Default for InputInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl InputInjector {
    /// 初始化注入器，探测 /dev/uinput；无权限时建立 X11 连接作为回退通道
    pub fn new() -> Self {
        let mut keys = AttributeSet::<Key>::new();
        keys.insert(Key::KEY_LEFTCTRL);
        keys.insert(Key::KEY_RIGHTCTRL);
        keys.insert(Key::KEY_V);
        keys.insert(Key::KEY_LEFTSHIFT);
        keys.insert(Key::KEY_INSERT);
        keys.insert(Key::KEY_ENTER);

        let (uinput_device, has_uinput_permission) = match VirtualDeviceBuilder::new()
            .and_then(|b| b.name("Vok Virtual Keyboard").with_keys(&keys)?.build())
        {
            Ok(dev) => {
                info!("【硬件直接粘贴】成功建立 Linux 内核 /dev/uinput 虚拟键盘设备");
                (Some(dev), true)
            }
            Err(e) => {
                debug!("未能打开 /dev/uinput (原因: {:?})，将使用 X11 XTEST 协议作为直接粘贴通道", e);
                (None, false)
            }
        };

        let x11_conn = x11rb::connect(None).ok();

        Self {
            uinput_device,
            has_uinput_permission,
            x11_conn,
        }
    }

    /// 是否具备 /dev/uinput 原生硬件写权限
    pub fn has_uinput_permission(&self) -> bool {
        self.has_uinput_permission
    }

    /// 注入直接粘贴组合键 (Ctrl + V)
    /// 遵循双引擎策略：优先内核 uinput；回退 X11 XTEST
    pub fn inject_paste(&mut self) -> Result<(), InputError> {
        // 1. 优先尝试 /dev/uinput 内核设备
        if let Some(ref mut dev) = self.uinput_device {
            let press_ctrl = InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), 1);
            let press_v = InputEvent::new(EventType::KEY, Key::KEY_V.code(), 1);
            let rel_v = InputEvent::new(EventType::KEY, Key::KEY_V.code(), 0);
            let rel_ctrl = InputEvent::new(EventType::KEY, Key::KEY_LEFTCTRL.code(), 0);

            dev.emit(&[press_ctrl, press_v, rel_v, rel_ctrl])
                .map_err(|e| InputError::UInputError(e.to_string()))?;
            info!("【硬件直接粘贴】已通过 /dev/uinput 注入 Ctrl+V 物理击键");
            return Ok(());
        }

        // 2. 回退 X11 XTEST 扩展通道
        self.inject_x11_paste()
    }

    /// 通过 X11 XTEST 扩展注入 Control_L + V
    fn inject_x11_paste(&mut self) -> Result<(), InputError> {
        if self.x11_conn.is_none() {
            self.x11_conn = x11rb::connect(None).ok();
        }

        let (ref conn, screen_num) = self
            .x11_conn
            .as_ref()
            .ok_or_else(|| InputError::X11Error("无法建立 X11 连接".to_string()))?;
        let screen = &conn.setup().roots[*screen_num];

        // 在标准 X11 PC105 键盘模型下：
        // Control_L keycode 为 37 (evdev 29 + 8)
        // 'v' / 'V' keycode 为 55 (evdev 47 + 8)
        let ctrl_keycode: u8 = 37;
        let v_keycode: u8 = 55;

        // 依次发送 KeyPress(Ctrl) -> KeyPress(V) -> KeyRelease(V) -> KeyRelease(Ctrl)
        conn.xtest_fake_input(KEY_PRESS_EVENT, ctrl_keycode, 0, screen.root, 0, 0, 0)
            .map_err(|e| InputError::X11Error(e.to_string()))?;
        conn.xtest_fake_input(KEY_PRESS_EVENT, v_keycode, 0, screen.root, 0, 0, 0)
            .map_err(|e| InputError::X11Error(e.to_string()))?;
        conn.xtest_fake_input(KEY_RELEASE_EVENT, v_keycode, 0, screen.root, 0, 0, 0)
            .map_err(|e| InputError::X11Error(e.to_string()))?;
        conn.xtest_fake_input(KEY_RELEASE_EVENT, ctrl_keycode, 0, screen.root, 0, 0, 0)
            .map_err(|e| InputError::X11Error(e.to_string()))?;

        conn.flush()
            .map_err(|e| InputError::X11Error(e.to_string()))?;

        info!("【硬件直接粘贴】已通过 X11 XTEST 协议注入 Ctrl+V 击键");
        Ok(())
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

    #[test]
    fn test_input_injector_creation_and_paste_does_not_panic() {
        let mut injector = InputInjector::new();
        // 验证有权限或无权限状态下构建均不 Panic
        let _has_perm = injector.has_uinput_permission();
        // 尝试执行注入，验证降级通道可用性
        let res = injector.inject_paste();
        // 在 X11 测试环境中应当成功（通过 uinput 或 XTEST）
        assert!(res.is_ok(), "按键注入应当成功 (uinput 或 XTEST 兜底): {:?}", res);
    }

    #[test]
    fn test_key_name_to_evdev_key_mapping() {
        assert_eq!(key_name_to_evdev_key("F12"), Some(Key::KEY_F12));
        assert_eq!(key_name_to_evdev_key("space"), Some(Key::KEY_SPACE));
        assert_eq!(key_name_to_evdev_key("Space"), Some(Key::KEY_SPACE));
        assert_eq!(key_name_to_evdev_key("k"), Some(Key::KEY_K));
        assert_eq!(key_name_to_evdev_key("K"), Some(Key::KEY_K));
        assert_eq!(key_name_to_evdev_key("return"), Some(Key::KEY_ENTER));
        assert_eq!(key_name_to_evdev_key("enter"), Some(Key::KEY_ENTER));
        assert_eq!(key_name_to_evdev_key("escape"), Some(Key::KEY_ESC));
        assert_eq!(key_name_to_evdev_key("esc"), Some(Key::KEY_ESC));
        assert_eq!(key_name_to_evdev_key("`"), Some(Key::KEY_GRAVE));
        assert_eq!(key_name_to_evdev_key("~"), Some(Key::KEY_GRAVE));
        assert_eq!(key_name_to_evdev_key("unknown_key_xyz"), None);
    }

    #[test]
    fn test_modifier_tracker_matching() {
        let tracker = ModifierTracker::new();
        let target_alt_space = ModifierMask::new(false, true, false, false);
        assert!(!tracker.matches(&target_alt_space));

        // 按下 Left Alt
        tracker.update_key(Key::KEY_LEFTALT.code(), true);
        assert!(tracker.matches(&target_alt_space));

        // 同时按下了 Ctrl -> 不匹配
        tracker.update_key(Key::KEY_LEFTCTRL.code(), true);
        assert!(!tracker.matches(&target_alt_space));

        // 松开 Ctrl -> 再次匹配
        tracker.update_key(Key::KEY_LEFTCTRL.code(), false);
        assert!(tracker.matches(&target_alt_space));

        // 松开 Alt -> 不匹配
        tracker.update_key(Key::KEY_LEFTALT.code(), false);
        assert!(!tracker.matches(&target_alt_space));
    }

    #[test]
    fn test_evdev_device_read() {
        let dev = Device::open("/dev/input/event2");
        println!("dev result: {:?}", dev.is_ok());
        if let Ok(d) = dev {
            println!("name: {:?}", d.name());
            let keys = d.supported_keys();
            println!("supported keys: {:?}", keys.is_some());
            if let Some(k) = keys {
                println!("contains KEY_LEFTALT: {}", k.contains(Key::KEY_LEFTALT));
                println!("contains KEY_SPACE: {}", k.contains(Key::KEY_SPACE));
                println!("KEY_LEFTALT code: {}", Key::KEY_LEFTALT.code());
                println!("KEY_SPACE code: {}", Key::KEY_SPACE.code());
            }
        }
    }
}
