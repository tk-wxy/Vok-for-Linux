use std::path::{Path, PathBuf};
use thiserror::Error;

/// 启动台条目模型 (Launcher Item)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherItem {
    /// 唯一标识
    pub id: i64,
    /// 应用程序或文件显示名称 (如 "Firefox", "工作周报.xlsx")
    pub title: String,
    /// 执行目标：.desktop 文件 ID、可执行文件绝对路径或文档路径
    pub target: String,
    /// XDG 图标名称或图标文件的绝对路径
    pub icon_name: String,
    /// 排序序号 (从 0 开始连续)
    pub order_index: i32,
    /// 创建毫秒时间戳
    pub created_at_ms: i64,
}

impl LauncherItem {
    pub fn new(
        id: i64,
        title: impl Into<String>,
        target: impl Into<String>,
        icon_name: impl Into<String>,
        order_index: i32,
        created_at_ms: i64,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            target: target.into(),
            icon_name: icon_name.into(),
            order_index,
            created_at_ms,
        }
    }

    /// 校验条目是否合法 (标题与目标均非空)
    pub fn is_valid(&self) -> bool {
        !self.title.trim().is_empty() && !self.target.trim().is_empty()
    }
}

/// 中转站条目类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageItemKind {
    /// 普通文件或文档
    File,
    /// 图片文件 (支持生成/展示图像缩略图)
    Image,
    /// 纯文本或富文本片段
    Text,
}

impl StageItemKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Image => "image",
            Self::Text => "text",
        }
    }

    pub fn from_str_kind(s: &str) -> Option<Self> {
        match s {
            "file" => Some(Self::File),
            "image" => Some(Self::Image),
            "text" => Some(Self::Text),
            _ => None,
        }
    }

    /// 根据 MIME 类型自动推断条目类型
    pub fn from_mime_type(mime: &str) -> Self {
        if mime.starts_with("image/") {
            Self::Image
        } else if mime.starts_with("text/") {
            Self::Text
        } else {
            Self::File
        }
    }
}

/// 中转站条目模型 (Stage Item)
///
/// 遵循 [R4 引用型货架原则]：仅保存本地物理路径与元数据引用，严禁擅自修改原文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageItem {
    /// 唯一标识
    pub id: i64,
    /// 条目类型 (File, Image, Text)
    pub kind: StageItemKind,
    /// 规范化 file:// URI
    pub uri: String,
    /// 本地物理路径引用
    pub path: PathBuf,
    /// 文件名或文本内容摘要
    pub display_name: String,
    /// 文件大小 (字节)
    pub size_bytes: u64,
    /// MIME 类型 (如 "image/png", "application/pdf")
    pub mime_type: String,
    /// 文件最近修改时间 (秒级时间戳)
    pub mtime_sec: u64,
    /// 异步生成的 160px 缩略图物理绝对路径
    pub thumbnail: Option<PathBuf>,
    /// 是否锁定置顶
    pub pinned: bool,
    /// 排序序号 (从 0 开始连续)
    pub order_index: i32,
    /// 录入时间戳 (毫秒)
    pub created_at_ms: i64,
}

impl StageItem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: i64,
        kind: StageItemKind,
        uri: impl Into<String>,
        path: impl Into<PathBuf>,
        display_name: impl Into<String>,
        size_bytes: u64,
        mime_type: impl Into<String>,
        mtime_sec: u64,
        thumbnail: Option<PathBuf>,
        pinned: bool,
        order_index: i32,
        created_at_ms: i64,
    ) -> Self {
        Self {
            id,
            kind,
            uri: uri.into(),
            path: path.into(),
            display_name: display_name.into(),
            size_bytes,
            mime_type: mime_type.into(),
            mtime_sec,
            thumbnail,
            pinned,
            order_index,
            created_at_ms,
        }
    }

    /// 从本地物理路径构建引用型条目 (不会进行任何文件移动或写入)
    #[allow(clippy::too_many_arguments)]
    pub fn from_path_ref(
        id: i64,
        path: &Path,
        display_name: impl Into<String>,
        size_bytes: u64,
        mime_type: impl Into<String>,
        mtime_sec: u64,
        order_index: i32,
        created_at_ms: i64,
    ) -> Self {
        let mime_type = mime_type.into();
        let kind = StageItemKind::from_mime_type(&mime_type);
        let uri = format!("file://{}", path.to_string_lossy());
        Self {
            id,
            kind,
            uri,
            path: path.to_path_buf(),
            display_name: display_name.into(),
            size_bytes,
            mime_type,
            mtime_sec,
            thumbnail: None,
            pinned: false,
            order_index,
            created_at_ms,
        }
    }
}

/// 列表排序计算错误
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReorderError {
    #[error("Index out of bounds: index {index}, length {length}")]
    IndexOutOfBounds { index: usize, length: usize },
    #[error("List is empty")]
    EmptyList,
}

/// 计算列表元素从 `from_index` 拖拽移动到 `to_index` 后的最小增量变更集合
///
/// 返回 `Vec<(id, new_order_index)>`，仅包含排序位置发生改变的条目，方便数据库批量更新。
pub fn calculate_reorder(
    current_ids: &[i64],
    from_index: usize,
    to_index: usize,
) -> Result<Vec<(i64, i32)>, ReorderError> {
    let len = current_ids.len();
    if len == 0 {
        return Err(ReorderError::EmptyList);
    }
    if from_index >= len {
        return Err(ReorderError::IndexOutOfBounds {
            index: from_index,
            length: len,
        });
    }
    if to_index >= len {
        return Err(ReorderError::IndexOutOfBounds {
            index: to_index,
            length: len,
        });
    }
    if from_index == to_index {
        return Ok(Vec::new());
    }

    let mut reordered = current_ids.to_vec();
    let moved_id = reordered.remove(from_index);
    reordered.insert(to_index, moved_id);

    let start = from_index.min(to_index);
    let end = from_index.max(to_index);

    let mut changes = Vec::with_capacity(end - start + 1);
    for (i, &id) in reordered.iter().enumerate().take(end + 1).skip(start) {
        if id != current_ids[i] {
            changes.push((id, i as i32));
        }
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launcher_item_creation_and_validation() {
        let item = LauncherItem::new(
            1,
            "Firefox",
            "firefox.desktop",
            "firefox",
            0,
            1726000000000,
        );
        assert_eq!(item.id, 1);
        assert_eq!(item.title, "Firefox");
        assert!(item.is_valid());

        let invalid_item = LauncherItem::new(2, "", "target", "icon", 1, 0);
        assert!(!invalid_item.is_valid());
    }

    #[test]
    fn test_stage_item_kind_and_reference() {
        let img_kind = StageItemKind::from_mime_type("image/png");
        assert_eq!(img_kind, StageItemKind::Image);
        assert_eq!(img_kind.as_str(), "image");

        let pdf_kind = StageItemKind::from_mime_type("application/pdf");
        assert_eq!(pdf_kind, StageItemKind::File);

        let txt_kind = StageItemKind::from_mime_type("text/plain");
        assert_eq!(txt_kind, StageItemKind::Text);

        let path = Path::new("/tmp/test_file.png");
        let stage_item = StageItem::from_path_ref(
            10,
            path,
            "test_file.png",
            2048,
            "image/png",
            1726000000,
            0,
            1726000000000,
        );
        assert_eq!(stage_item.kind, StageItemKind::Image);
        assert_eq!(stage_item.uri, "file:///tmp/test_file.png");
        assert_eq!(stage_item.path, path);
        assert_eq!(stage_item.size_bytes, 2048);
        assert!(!stage_item.pinned);
    }

    #[test]
    fn test_calculate_reorder_forward() {
        // [10, 20, 30, 40] 移动 index 0 (10) 到 index 2
        // 期望结果：[20, 30, 10, 40]
        // 发生位置变更的项：20 (变成 0), 30 (变成 1), 10 (变成 2)
        let ids = [10, 20, 30, 40];
        let changes = calculate_reorder(&ids, 0, 2).expect("reorder should succeed");
        assert_eq!(
            changes,
            vec![(20, 0), (30, 1), (10, 2)]
        );
    }

    #[test]
    fn test_calculate_reorder_backward() {
        // [10, 20, 30, 40] 移动 index 3 (40) 到 index 1
        // 期望结果：[10, 40, 20, 30]
        // 发生位置变更的项：40 (变成 1), 20 (变成 2), 30 (变成 3)
        let ids = [10, 20, 30, 40];
        let changes = calculate_reorder(&ids, 3, 1).expect("reorder should succeed");
        assert_eq!(
            changes,
            vec![(40, 1), (20, 2), (30, 3)]
        );
    }

    #[test]
    fn test_calculate_reorder_no_op_and_boundary() {
        let ids = [1, 2, 3];
        // 相同位置无变更
        let changes = calculate_reorder(&ids, 1, 1).expect("no-op should succeed");
        assert!(changes.is_empty());

        // 越界保护
        let err = calculate_reorder(&ids, 0, 5).unwrap_err();
        assert_eq!(
            err,
            ReorderError::IndexOutOfBounds {
                index: 5,
                length: 3
            }
        );

        // 空列表
        let empty: [i64; 0] = [];
        let err_empty = calculate_reorder(&empty, 0, 0).unwrap_err();
        assert_eq!(err_empty, ReorderError::EmptyList);
    }
}
