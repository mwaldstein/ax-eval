use crate::run::utils::copy_dir_recursive;
use crate::utils::resolve_fixtures_path;
use std::fs;
use std::path::{Path, PathBuf};

pub struct TestEnv {
    pub root: PathBuf,
}

impl TestEnv {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        if root.exists() {
            fs::remove_dir_all(&root)?;
        }
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn setup_fixture_relative_to(
        &self,
        fixture_name: &str,
        scenario_dir: Option<&Path>,
    ) -> anyhow::Result<()> {
        let fixture_path = Path::new(fixture_name);
        let local_fixture = scenario_dir
            .filter(|_| !fixture_path.is_absolute())
            .map(|dir| dir.join(fixture_path))
            .filter(|path| path.exists());
        let fixture_src = local_fixture.unwrap_or_else(|| {
            if fixture_path.is_absolute() {
                fixture_path.to_path_buf()
            } else {
                resolve_fixtures_path("templates").join(fixture_path)
            }
        });
        if !fixture_src.exists() {
            anyhow::bail!("Fixture not found: {:?}", fixture_src);
        }
        copy_dir_recursive(&fixture_src, &self.root)?;
        Ok(())
    }
}
