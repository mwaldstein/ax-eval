use crate::results::{Cache, CacheKey, ResultRecord};
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub fn compute_cache_key_with_fixture(
    scenario_yaml: &str,
    prompt: &str,
    fixture_root: &Path,
    tool: &str,
    model: &str,
) -> anyhow::Result<CacheKey> {
    let fixture_hash = hash_fixture_contents(fixture_root)?;
    Ok(CacheKey::compute_with_fixture(
        scenario_yaml,
        prompt,
        &fixture_hash,
        tool,
        model,
    ))
}

pub fn check_cache(cache: &Cache, cache_key: &CacheKey) -> anyhow::Result<Option<ResultRecord>> {
    Ok(cache.get(cache_key))
}

fn hash_fixture_contents(root: &Path) -> anyhow::Result<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    for relative_path in files {
        hasher.update(relative_path.to_string_lossy().as_bytes());
        hasher.update([0]);
        let full_path = root.join(&relative_path);
        let content = std::fs::read(&full_path)
            .with_context(|| format!("Failed to read fixture file {}", full_path.display()))?;
        hasher.update(content);
        hasher.update([0]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read fixture directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            files.push(path.strip_prefix(root)?.to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_hash_changes_when_file_content_changes() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();
        std::fs::write(root.join("AGENTS.md"), "first").expect("write fixture file");

        let first = hash_fixture_contents(root).expect("hash fixture");
        std::fs::write(root.join("AGENTS.md"), "second").expect("update fixture file");
        let second = hash_fixture_contents(root).expect("hash updated fixture");

        assert_ne!(first, second);
    }

    #[test]
    fn fixture_hash_changes_when_file_path_changes() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let root = temp.path();
        std::fs::write(root.join("one.txt"), "same").expect("write fixture file");
        let first = hash_fixture_contents(root).expect("hash fixture");

        std::fs::remove_file(root.join("one.txt")).expect("remove fixture file");
        std::fs::write(root.join("two.txt"), "same").expect("write renamed fixture file");
        let second = hash_fixture_contents(root).expect("hash updated fixture");

        assert_ne!(first, second);
    }
}
