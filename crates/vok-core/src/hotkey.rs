use std::time::Duration;

/// 热键触发模式判决阈值 (毫秒)
/// 
/// 按住物理按键超过此时间后松开被视作长按瞬态（Momentary，松手即隐），
/// 小于等于此时间被视作短按翻转（Toggle，松手保持显示）。
pub const DEFAULT_TAP_MAX_MS: u64 = 250;

/// 热键工作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyMode {
    /// 瞬态长按模式：按下立即揭示面板，松开瞬间立即隐藏（主要用于抓取外部文件临时丢入中转站）
    Momentary,
    /// 保持翻转模式：短按松开后常驻显示；再次按下或按 Esc 隐藏
    Toggle,
}

impl HotkeyMode {
    /// 根据物理按下至释放的持续时间判决行为模式
    pub fn from_duration(duration: Duration, threshold: Duration) -> Self {
        if duration > threshold {
            HotkeyMode::Momentary
        } else {
            HotkeyMode::Toggle
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotkey_mode_decision() {
        let threshold = Duration::from_millis(DEFAULT_TAP_MAX_MS);

        // 持续 50ms：短按
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(50), threshold),
            HotkeyMode::Toggle
        );

        // 持续 250ms：边界仍然判定为 Toggle
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(250), threshold),
            HotkeyMode::Toggle
        );

        // 持续 251ms：长按判定为 Momentary
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(251), threshold),
            HotkeyMode::Momentary
        );

        // 持续 1000ms：长按 Momentary
        assert_eq!(
            HotkeyMode::from_duration(Duration::from_millis(1000), threshold),
            HotkeyMode::Momentary
        );
    }
}
