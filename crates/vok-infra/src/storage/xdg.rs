use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

/// 获取用户 Home 目录
fn get_home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// 获取 Vok 数据存储根目录 ($XDG_DATA_HOME/vok，默认 ~/.local/share/vok)
pub fn get_vok_data_dir() -> PathBuf {
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| get_home_dir().join(".local").join("share"))
        .join("vok")
}

/// 获取 Vok 主数据库文件绝对路径 ($XDG_DATA_HOME/vok/vok.db)
pub fn get_vok_db_path() -> PathBuf {
    get_vok_data_dir().join("vok.db")
}

/// 获取 Vok 缓存根目录 ($XDG_CACHE_HOME/vok，默认 ~/.cache/vok)
pub fn get_vok_cache_dir() -> PathBuf {
    env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| get_home_dir().join(".cache"))
        .join("vok")
}

/// 获取 Vok 缩略图缓存目录 ($XDG_CACHE_HOME/vok/thumbs)
pub fn get_vok_thumbs_dir() -> PathBuf {
    get_vok_cache_dir().join("thumbs")
}

/// 自动递归创建 Vok 所需的本地数据与缓存目录
pub fn ensure_storage_dirs() -> io::Result<()> {
    let data_dir = get_vok_data_dir();
    if !data_dir.exists() {
        fs::create_dir_all(&data_dir)?;
    }

    let thumbs_dir = get_vok_thumbs_dir();
    if !thumbs_dir.exists() {
        fs::create_dir_all(&thumbs_dir)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xdg_paths_resolution() {
        let db_path = get_vok_db_path();
        assert!(db_path.ends_with("vok/vok.db"));

        let thumbs_path = get_vok_thumbs_dir();
        assert!(thumbs_path.ends_with("vok/thumbs"));
    }
}
