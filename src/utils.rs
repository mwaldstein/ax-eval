use crate::config::Config;
use std::path::{Path, PathBuf};

pub fn resolve_fixtures_path(relative_path: &str) -> PathBuf {
    if Path::new(relative_path).is_absolute() || Path::new(relative_path).exists() {
        PathBuf::from(relative_path)
    } else {
        // Use config if available, otherwise default to fixtures
        let config = Config::load_or_default();
        let base_path = config.get_fixtures_path();
        PathBuf::from(base_path).join(relative_path)
    }
}
