use super::{
    claude_code::ClaudeCodeAdapter, codex::CodexAdapter, mock::MockAdapter,
    opencode::OpenCodeAdapter, ToolAdapter,
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

#[derive(Clone)]
pub struct CheckedAdapter {
    tool: String,
    adapter: Arc<dyn ToolAdapter>,
}

impl CheckedAdapter {
    pub fn tool(&self) -> &str {
        &self.tool
    }

    pub fn adapter(&self) -> &dyn ToolAdapter {
        self.adapter.as_ref()
    }
}

enum AdapterEntry {
    Available(CheckedAdapter),
    Unavailable(String),
}

#[derive(Default)]
pub struct AdapterRegistry {
    checked: HashMap<String, AdapterEntry>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn resolve_checked(&mut self, tool: &str) -> Result<CheckedAdapter> {
        if let Some(entry) = self.checked.get(tool) {
            return match entry {
                AdapterEntry::Available(adapter) => Ok(adapter.clone()),
                AdapterEntry::Unavailable(error) => anyhow::bail!("{error}"),
            };
        }

        let adapter = match create_adapter(tool) {
            Ok(adapter) => adapter,
            Err(error) => {
                let message = format!("{error:#}");
                debug!("failed to create adapter for {}: {}", tool, message);
                self.checked
                    .insert(tool.to_string(), AdapterEntry::Unavailable(message.clone()));
                anyhow::bail!("{message}");
            }
        };
        debug!("checking availability for tool: {}", tool);
        if let Err(error) = adapter.check_availability() {
            let message = format!("{error:#}");
            self.checked
                .insert(tool.to_string(), AdapterEntry::Unavailable(message.clone()));
            anyhow::bail!("{message}");
        }

        let adapter = Arc::from(adapter);
        let checked = CheckedAdapter {
            tool: tool.to_string(),
            adapter,
        };
        self.checked
            .insert(tool.to_string(), AdapterEntry::Available(checked.clone()));

        Ok(checked)
    }
}

pub fn create_adapter(tool: &str) -> Result<Box<dyn ToolAdapter>> {
    let adapter: Box<dyn ToolAdapter> = match tool {
        "claude" | "claude-code" => Box::new(ClaudeCodeAdapter),
        "codex" => Box::new(CodexAdapter),
        "mock" => Box::new(MockAdapter),
        "opencode" => Box::new(OpenCodeAdapter),
        _ => anyhow::bail!("Unknown tool: {}", tool),
    };
    Ok(adapter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_reuses_checked_adapter() {
        let mut registry = AdapterRegistry::new();

        let first = registry.resolve_checked("mock").expect("first adapter");
        let second = registry.resolve_checked("mock").expect("second adapter");

        assert_eq!(first.tool(), "mock");
        assert!(Arc::ptr_eq(&first.adapter, &second.adapter));
    }

    #[test]
    fn registry_reuses_unknown_tool_failure() {
        let mut registry = AdapterRegistry::new();

        let first = match registry.resolve_checked("unknown-tool") {
            Ok(_) => panic!("first resolution should fail"),
            Err(error) => error.to_string(),
        };
        let second = match registry.resolve_checked("unknown-tool") {
            Ok(_) => panic!("second resolution should fail"),
            Err(error) => error.to_string(),
        };

        assert_eq!(first, second);
        assert!(first.contains("Unknown tool"));
    }
}
