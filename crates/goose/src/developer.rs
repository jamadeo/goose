use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use indoc::{indoc, formatdoc};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use crate::errors::{AgentError, AgentResult};
use crate::models::content::Content;
use crate::models::tool::{Tool, ToolCall};
use crate::systems::System;

pub struct DeveloperSystem {
    tools: Vec<Tool>,
    cwd: Mutex<PathBuf>,
    active_files: Mutex<HashSet<PathBuf>>,
    file_history: Mutex<HashMap<PathBuf, Vec<String>>>, // Moved file_history here
}

impl Default for DeveloperSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DeveloperSystem {
    pub fn new() -> Self {
        // TODO i suggest we make working_dir required, we're seeing abs paths are more clear
        let bash_tool = Tool::new(
            "bash",
            indoc! {r#"
                Run a bash command in the shell in the current working directory
                  - You can use multiline commands or && to execute multiple in one pass
                  - Directory changes **are not** persisted from one command to the next
                  - Sourcing files **is not** persisted from one command to the next

                For example, you can use this style to execute python in a virtualenv
                "source .venv/bin/active && python example1.py"

                but need to repeat the source for subsequent commands in that virtualenv
                "source .venv/bin/active && python example2.py"
            "#},
            json!({
                "type": "object",
                "required": [],
                "properties": {
                    "command": {
                        "type": "string",
                        "default": null,
                        "description": "The bash shell command to run."
                    },
                }
            }),
        );

        let text_editor_tool = Tool::new(
            "text_editor",
            indoc! {r#"
                Perform text editing operations on files.
                The `command` parameter specifies the operation to perform.
            "#},
            json!({
                "type": "object",
                "required": ["command", "path"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file. Can be absolute or relative to the system CWD"
                    },
                    "command": {
                        "enum": ["view", "create", "replace", "insert", "undo"],
                        "description": "The commands to run."
                    },
                    "new_str": {
                        "type": "string",
                        "default": null,
                        "description": "Required for `replace` and `insert` commands."
                    },
                    "old_str": {
                        "type": "string",
                        "default": null,
                        "description": "Required for `str_replace` command."
                    },
                    "file_text": {
                        "type": "string",
                        "default": null,
                        "description": "Required for `create` command."
                    },
                    "insert_line": {
                        "type": "integer",
                        "default": null,
                        "description": "Required for `insert` command."
                    }
                }
            }),
        );

        Self {
            tools: vec![bash_tool, text_editor_tool],
            cwd: Mutex::new(std::env::current_dir().unwrap()),
            active_files: Mutex::new(HashSet::new()),
            file_history: Mutex::new(HashMap::new()), // Initialize file_history
        }
    }

    // Helper method to resolve a path relative to cwd
    fn resolve_path(&self, path_str: &str) -> AgentResult<PathBuf> {
        let cwd = self.cwd.lock().unwrap();
        let path = Path::new(path_str);
        let resolved_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };

        Ok(resolved_path)
    }

    // Implement bash tool functionality
    async fn bash(&self, params: Value) -> AgentResult<Vec<Content>> {
        let command =
            params
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or(AgentError::InvalidParameters(
                    "command argument as a string is required".into(),
                ))?;

        // Disallow commands that should use other tools
        if command.trim_start().starts_with("cat") {
            return Err(AgentError::InvalidParameters(
                "Do not use `cat` to read files, use the view mode on the text editor tool"
                    .to_string(),
            ));
        }
        // TODO consider enforcing ripgrep over find?

        // Redirect stderr to stdout to interleave outputs
        let cmd_with_redirect = format!("{} 2>&1", command);

        // Execute the command
        let output = Command::new("bash")
            .arg("-c")
            .arg(cmd_with_redirect)
            .output()
            .map_err(|e| AgentError::ExecutionError(e.to_string()))?;

        let output_str = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            return Err(AgentError::ExecutionError(output_str));
        }

        let formatted = formatdoc!{"
            ## Output

            ```
            {}
            ```
            ",
            output_str
        };

        Ok(vec![Content::text(formatted)])
    }

    // Implement text_editor tool functionality
    async fn text_editor(&self, params: Value) -> AgentResult<Vec<Content>> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'command' parameter".into()))?;

        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'path' parameter".into()))?;

        let path = self.resolve_path(path_str)?;

        match command {
            "view" => self.text_editor_view(&path).await,
            "create" => {
                let file_text = params
                    .get("file_text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InvalidParameters("Missing 'file_text' parameter".into())
                    })?;

                self.text_editor_create(&path, file_text).await
            }
            "replace" => {
                let old_str = params
                    .get("old_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InvalidParameters("Missing 'old_str' parameter".into())
                    })?;
                let new_str = params
                    .get("new_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InvalidParameters("Missing 'new_str' parameter".into())
                    })?;

                self.text_editor_replace(&path, old_str, new_str).await
            }
            "insert" => {
                let insert_line = params
                    .get("insert_line")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        AgentError::InvalidParameters("Missing 'insert_line' parameter".into())
                    })?;
                let new_str = params
                    .get("new_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InvalidParameters("Missing 'new_str' parameter".into())
                    })?;

                self.text_editor_insert(&path, insert_line as usize, new_str)
                    .await
            }
            "undo" => self.text_editor_undo(&path).await,
            _ => Err(AgentError::InvalidParameters(format!(
                "Unknown command '{}'",
                command
            ))),
        }
    }

    async fn text_editor_view(&self, path: &PathBuf) -> AgentResult<Vec<Content>> {
        if path.is_file() {
            // Rather than reading, we add this to the active files and it is shown in the system status

            // Add to active files
            self.active_files.lock().unwrap().insert(path.clone());

            Ok(vec![Content::text(format!(
                "The file content for {} is now available in the system status.",
                path.display()
            ))])
        } else {
            Err(AgentError::InvalidParameters(format!(
                "The path '{}' does not exist or is not a file.",
                path.display()
            )))
        }
    }

    async fn text_editor_create(
        &self,
        path: &PathBuf,
        file_text: &str,
    ) -> AgentResult<Vec<Content>> {
        // Check if file already exists and is active
        if path.exists() && !self.active_files.lock().unwrap().contains(path) {
            return Err(AgentError::InvalidParameters(format!(
                "File '{}' exists but is not active. View it first before overwriting.",
                path.display()
            )));
        }

        // Save history for undo
        self.save_file_history(path)?;

        // Write to the file
        std::fs::write(path, file_text)
            .map_err(|e| AgentError::ExecutionError(format!("Failed to write file: {}", e)))?;

        // Add to active files
        self.active_files.lock().unwrap().insert(path.clone());

        Ok(vec![Content::text(format!(
            "Successfully wrote to {}",
            path.display()
        ))])
    }

    async fn text_editor_replace(
        &self,
        path: &PathBuf,
        old_str: &str,
        new_str: &str,
    ) -> AgentResult<Vec<Content>> {
        // Check if file exists and is active
        if !path.exists() {
            return Err(AgentError::InvalidParameters(format!(
                "File '{}' does not exist",
                path.display()
            )));
        }
        if !self.active_files.lock().unwrap().contains(path) {
            return Err(AgentError::InvalidParameters(format!(
                "You must view '{}' before editing it",
                path.display()
            )));
        }

        // Read content
        let content = std::fs::read_to_string(path)
            .map_err(|e| AgentError::ExecutionError(format!("Failed to read file: {}", e)))?;

        // Ensure 'old_str' appears exactly once
        if content.matches(old_str).count() != 1 {
            return Err(AgentError::InvalidParameters(
                "'old_str' must appear exactly once in the file".into(),
            ));
        }

        // Save history for undo
        self.save_file_history(path)?;

        // Replace and write back
        let new_content = content.replace(old_str, new_str);
        std::fs::write(path, new_content)
            .map_err(|e| AgentError::ExecutionError(format!("Failed to write file: {}", e)))?;

        Ok(vec![Content::text("Successfully replaced text")])
    }

    async fn text_editor_insert(
        &self,
        path: &PathBuf,
        insert_line: usize,
        new_str: &str,
    ) -> AgentResult<Vec<Content>> {
        // Check if file exists and is active
        if !path.exists() {
            return Err(AgentError::InvalidParameters(format!(
                "File '{}' does not exist",
                path.display()
            )));
        }
        if !self.active_files.lock().unwrap().contains(path) {
            return Err(AgentError::InvalidParameters(format!(
                "You must view '{}' before editing it",
                path.display()
            )));
        }

        // Read lines
        let content = std::fs::read_to_string(path)
            .map_err(|e| AgentError::ExecutionError(format!("Failed to read file: {}", e)))?;
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        if insert_line > lines.len() {
            return Err(AgentError::InvalidParameters(
                "Insert line number is out of range".into(),
            ));
        }

        // Save history for undo
        self.save_file_history(path)?;

        // Insert new string after the specified line
        lines.insert(insert_line, new_str.to_string());

        // Write back to file
        std::fs::write(path, lines.join("\n"))
            .map_err(|e| AgentError::ExecutionError(format!("Failed to write file: {}", e)))?;

        Ok(vec![Content::text("Successfully inserted text")])
    }

    async fn text_editor_undo(&self, path: &PathBuf) -> AgentResult<Vec<Content>> {
        let mut history = self.file_history.lock().unwrap();
        if let Some(contents) = history.get_mut(path) {
            if let Some(previous_content) = contents.pop() {
                // Write previous content back to file
                std::fs::write(path, previous_content).map_err(|e| {
                    AgentError::ExecutionError(format!("Failed to write file: {}", e))
                })?;
                Ok(vec![Content::text("Successfully undid the last edit")])
            } else {
                Err(AgentError::InvalidParameters(
                    "No edit history available to undo".into(),
                ))
            }
        } else {
            Err(AgentError::InvalidParameters(
                "No edit history available to undo".into(),
            ))
        }
    }

    fn save_file_history(&self, path: &PathBuf) -> AgentResult<()> {
        let mut history = self.file_history.lock().unwrap();
        let content = if path.exists() {
            std::fs::read_to_string(path)
                .map_err(|e| AgentError::ExecutionError(format!("Failed to read file: {}", e)))?
        } else {
            String::new()
        };
        history.entry(path.clone()).or_default().push(content);
        Ok(())
    }
}

#[async_trait]
impl System for DeveloperSystem {
    fn name(&self) -> &str {
        "DeveloperSystem"
    }

    fn description(&self) -> &str {
        "A system that provides bash and text editing capabilities"
    }

    fn instructions(&self) -> &str {
        "Use the provided tools to perform bash operations and text editing tasks."
    }

    fn tools(&self) -> &[Tool] {
        &self.tools
    }

    async fn status(&self) -> AnyhowResult<HashMap<String, Value>> {
        let cwd = self.cwd.lock().unwrap().display().to_string();
        let mut file_contents = HashMap::new();

        // Get mutable access to active_files to remove any we can't read
        let mut active_files = self.active_files.lock().unwrap();

        // Use retain to keep only the files we can successfully read
        active_files.retain(|path| {
            if !path.exists() {
                return false;
            }

            match std::fs::read_to_string(path) {
                Ok(content) => {
                    file_contents.insert(path.display().to_string(), content);
                    true
                }
                Err(_) => false,
            }
        });

        Ok(HashMap::from([
            ("cwd".to_string(), json!(cwd)),
            ("files".to_string(), json!(file_contents)),
        ]))
    }

    async fn call(&self, tool_call: ToolCall) -> AgentResult<Vec<Content>> {
        match tool_call.name.as_str() {
            "bash" => self.bash(tool_call.arguments).await,
            "text_editor" => self.text_editor(tool_call.arguments).await,
            _ => Err(AgentError::ToolNotFound(tool_call.name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::sync::OnceCell;

    // Use OnceCell to initialize the system once for all tests
    static DEV_SYSTEM: OnceCell<DeveloperSystem> = OnceCell::const_new();

    async fn get_system() -> &'static DeveloperSystem {
        DEV_SYSTEM
            .get_or_init(|| async { DeveloperSystem::new() })
            .await
    }

    #[tokio::test]
    async fn test_bash_missing_parameters() {
        let system = get_system().await;

        let tool_call = ToolCall::new("bash", json!({"working_dir": "."}));
        let error = system.call(tool_call).await.unwrap_err();
        assert!(matches!(error, AgentError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_bash_change_directory() {
        let system = get_system().await;

        let tool_call = ToolCall::new("bash", json!({ "working_dir": ".", "command": "pwd" }));
        let result = system.call(tool_call).await.unwrap();
        assert!(result[0]
            .as_text()
            .unwrap()
            .contains(&std::env::current_dir().unwrap().display().to_string()));
    }

    #[tokio::test]
    async fn test_bash_invalid_directory() {
        let system = get_system().await;

        let tool_call = ToolCall::new("bash", json!({ "working_dir": "non_existent_dir" }));
        let error = system.call(tool_call).await.unwrap_err();
        assert!(matches!(error, AgentError::InvalidParameters(_)));
    }

    #[tokio::test]
    async fn test_text_editor_create_and_view_file() {
        let system = get_system().await;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        // Create a new file
        let create_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "create",
                "path": file_path_str,
                "file_text": "Hello, world!"
            }),
        );
        let create_result = system.call(create_call).await.unwrap();
        assert!(create_result[0]
            .as_text()
            .unwrap()
            .contains("Successfully wrote to"));

        // View the file
        let view_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "view",
                "path": file_path_str
            }),
        );
        let view_result = system.call(view_call).await.unwrap();
        assert_eq!(view_result[0].as_text().unwrap(), "Hello, world!");

        temp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_text_editor_str_replace() {
        let system = get_system().await;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        // Create a new file
        let create_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "create",
                "path": file_path_str,
                "file_text": "Hello, world!"
            }),
        );
        system.call(create_call).await.unwrap();

        // View the file to make it active
        let view_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "view",
                "path": file_path_str
            }),
        );
        system.call(view_call).await.unwrap();

        // Replace string
        let replace_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "str_replace",
                "path": file_path_str,
                "old_str": "world",
                "new_str": "Rust"
            }),
        );
        let replace_result = system.call(replace_call).await.unwrap();
        assert!(replace_result[0]
            .as_text()
            .unwrap()
            .contains("Successfully replaced text"));

        // View the file again
        let view_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "view",
                "path": file_path_str
            }),
        );
        let view_result = system.call(view_call).await.unwrap();
        assert_eq!(view_result[0].as_text().unwrap(), "Hello, Rust!");

        temp_dir.close().unwrap();
    }

    #[tokio::test]
    async fn test_text_editor_undo_edit() {
        let system = get_system().await;

        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let file_path_str = file_path.to_str().unwrap();

        // Create a new file
        let create_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "create",
                "path": file_path_str,
                "file_text": "First line"
            }),
        );
        system.call(create_call).await.unwrap();

        // View the file to make it active
        let view_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "view",
                "path": file_path_str
            }),
        );
        system.call(view_call).await.unwrap();

        // Insert a new line
        let insert_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "insert",
                "path": file_path_str,
                "insert_line": 1,
                "new_str": "Second line"
            }),
        );
        system.call(insert_call).await.unwrap();

        // Undo the edit
        let undo_call = ToolCall::new(
            "text_editor",
            json!({
                "command": "undo_edit",
                "path": file_path_str
            }),
        );
        let undo_result = system.call(undo_call).await.unwrap();
        assert!(undo_result[0]
            .as_text()
            .unwrap()
            .contains("Successfully undid the last edit"));

        // View the file again
        let view_result = system
            .call(ToolCall::new(
                "text_editor",
                json!({
                    "command": "view",
                    "path": file_path_str
                }),
            ))
            .await
            .unwrap();
        assert_eq!(view_result[0].as_text().unwrap(), "First line");

        temp_dir.close().unwrap();
    }
}
