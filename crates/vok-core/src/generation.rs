use std::sync::atomic::{AtomicU64, Ordering};

/// 代次令牌 (Generation ID)
/// 
/// 遵循规则 R6：每次工作台呼出必须生成单调递增的代次令牌。
/// 任何属于旧代次的异步回调（如缩略图计算完成、隐藏延时、旧丢放事件）一律丢弃，
/// 根除竞态时序污染。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationId(u64);

impl GenerationId {
    /// 创建初始代次（通常表示无活动会话）
    pub const fn initial() -> Self {
        Self(0)
    }

    /// 从原始数值构造（主要用于单测或序列化）
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// 获取底层原始代次数值
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// 判断当前代次是否有效（大于 0）
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }
}

impl std::fmt::Display for GenerationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gen#{}", self.0)
    }
}

/// 线程安全的代次分配器
#[derive(Debug, Default)]
pub struct GenerationIssuer {
    counter: AtomicU64,
}

impl GenerationIssuer {
    /// 创建新的分配器（从 1 开始单调递增）
    pub const fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// 生成下一个全新的有效代次令牌
    pub fn next(&self) -> GenerationId {
        let val = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        GenerationId(val)
    }

    /// 获取当前最新派发的代次数值（不自增）
    pub fn current(&self) -> GenerationId {
        GenerationId(self.counter.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_monotonic_increment() {
        let issuer = GenerationIssuer::new();
        assert_eq!(issuer.current(), GenerationId::initial());

        let g1 = issuer.next();
        let g2 = issuer.next();
        let g3 = issuer.next();

        assert_eq!(g1.raw(), 1);
        assert_eq!(g2.raw(), 2);
        assert_eq!(g3.raw(), 3);
        assert!(g1 < g2);
        assert!(g2 < g3);
        assert_eq!(issuer.current(), g3);
    }

    #[test]
    fn test_generation_display_and_validity() {
        let g0 = GenerationId::initial();
        assert!(!g0.is_valid());
        assert_eq!(format!("{g0}"), "Gen#0");

        let g1 = GenerationId::from_raw(42);
        assert!(g1.is_valid());
        assert_eq!(format!("{g1}"), "Gen#42");
    }
}
