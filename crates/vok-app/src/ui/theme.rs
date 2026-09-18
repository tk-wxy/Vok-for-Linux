//! 主题管理模块 (ThemeManager)
//!
//! 对接 Libadwaita StyleManager 与 SQLite app_settings，
//! 支持跟随系统、强制深色与强制浅色，并在系统日夜交替时自动热重载。

use std::cell::RefCell;
use std::rc::Rc;
use libadwaita as adw;
use tracing::info;
use vok_core::ThemeMode;
use vok_infra::storage::StorageClient;

pub type OnThemeChangedCallback = Box<dyn Fn(ThemeMode, bool) + 'static>;

#[derive(Clone)]
pub struct ThemeManager {
    storage: StorageClient,
    current_mode: Rc<RefCell<ThemeMode>>,
    style_manager: adw::StyleManager,
    listeners: Rc<RefCell<Vec<OnThemeChangedCallback>>>,
}

impl ThemeManager {
    /// 初始化主题管理器，从 SQLite 读取持久化配置并应用到 Libadwaita
    pub fn new(storage: StorageClient) -> Self {
        // 确保 Libadwaita 运行时已初始化
        let _ = adw::init();
        let style_manager = adw::StyleManager::default();

        // 1. 从 SQLite 读取 theme.mode (默认跟随系统)
        let mode = match storage.get_setting("theme.mode") {
            Ok(Some(val)) => ThemeMode::from_str_loose(&val),
            _ => ThemeMode::FollowSystem,
        };

        info!("【主题初始化】加载主题偏好: {} (存储值: {})", mode.display_name(), mode.as_str());

        // 2. 应用到 Libadwaita
        Self::apply_to_style_manager(&style_manager, mode);

        let current_mode = Rc::new(RefCell::new(mode));
        let listeners: Rc<RefCell<Vec<OnThemeChangedCallback>>> = Rc::new(RefCell::new(Vec::new()));

        // 3. 监听系统暗色状态变化 (特别是跟随系统时的日夜交替)
        {
            let listeners_clone = listeners.clone();
            let current_mode_clone = current_mode.clone();
            style_manager.connect_dark_notify(move |sm| {
                let is_dark = sm.is_dark();
                let mode = *current_mode_clone.borrow();
                info!("【主题事件】系统暗色属性发生变更 (is_dark={}), 当前模式={}", is_dark, mode.as_str());
                for cb in listeners_clone.borrow().iter() {
                    cb(mode, is_dark);
                }
            });
        }

        Self {
            storage,
            current_mode,
            style_manager,
            listeners,
        }
    }

    /// 设置新的主题模式并持久化至 SQLite
    pub fn set_mode(&self, mode: ThemeMode) {
        if *self.current_mode.borrow() == mode {
            return;
        }

        *self.current_mode.borrow_mut() = mode;
        if let Err(e) = self.storage.set_setting("theme.mode", mode.as_str()) {
            tracing::warn!("持久化主题配置失败: {:?}", e);
        }

        Self::apply_to_style_manager(&self.style_manager, mode);
        let is_dark = self.style_manager.is_dark();
        info!("【主题切换】已应用新主题: {} (实际暗色状态: {})", mode.display_name(), is_dark);

        for cb in self.listeners.borrow().iter() {
            cb(mode, is_dark);
        }
    }

    /// 获取当前生效的主题偏好模式
    pub fn current_mode(&self) -> ThemeMode {
        *self.current_mode.borrow()
    }

    /// 当前实际呈现是否为深色调
    pub fn is_dark(&self) -> bool {
        self.style_manager.is_dark()
    }

    /// 注册主题变更监听器
    pub fn connect_theme_changed<F: Fn(ThemeMode, bool) + 'static>(&self, callback: F) {
        self.listeners.borrow_mut().push(Box::new(callback));
    }

    fn apply_to_style_manager(sm: &adw::StyleManager, mode: ThemeMode) {
        match mode {
            ThemeMode::FollowSystem => {
                sm.set_color_scheme(adw::ColorScheme::Default);
            }
            ThemeMode::Dark => {
                sm.set_color_scheme(adw::ColorScheme::ForceDark);
            }
            ThemeMode::Light => {
                sm.set_color_scheme(adw::ColorScheme::ForceLight);
            }
        }
    }
}
