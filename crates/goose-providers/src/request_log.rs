use anyhow::{anyhow, Result};
use fs_err::File;
use goose_types::{ModelConfig, Usage};
use serde::Serialize;
use std::fmt::Display;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const LOGS_TO_KEEP: usize = 10;

pub struct RequestLog {
    writer: Option<BufWriter<File>>,
    logs_dir: PathBuf,
    temp_path: PathBuf,
}

impl RequestLog {
    pub fn start<Payload>(
        logs_dir: impl AsRef<Path>,
        model_config: &ModelConfig,
        payload: &Payload,
    ) -> Result<Self>
    where
        Payload: Serialize,
    {
        let logs_dir = logs_dir.as_ref().to_path_buf();
        fs_err::create_dir_all(&logs_dir)?;

        let request_id = Uuid::new_v4();
        let temp_path = logs_dir.join(format!("llm_request.{request_id}.jsonl"));

        let mut writer = BufWriter::new(
            File::options()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&temp_path)?,
        );

        let data = serde_json::json!({
            "model_config": model_config,
            "input": payload,
        });
        writeln!(writer, "{}", serde_json::to_string(&data)?)?;

        Ok(Self {
            writer: Some(writer),
            logs_dir,
            temp_path,
        })
    }

    fn write_json(&mut self, line: &serde_json::Value) -> Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| anyhow!("logger is finished"))?;
        writeln!(writer, "{}", serde_json::to_string(line)?)?;
        Ok(())
    }

    pub fn error<E>(&mut self, error: E) -> Result<()>
    where
        E: Display,
    {
        self.write_json(&serde_json::json!({
            "error": format!("{}", error),
        }))
    }

    pub fn write<Payload>(&mut self, data: &Payload, usage: Option<&Usage>) -> Result<()>
    where
        Payload: Serialize,
    {
        self.write_json(&serde_json::json!({
            "data": data,
            "usage": usage,
        }))
    }

    fn finish(&mut self) -> Result<()> {
        if let Some(mut writer) = self.writer.take() {
            writer.flush()?;
            let log_path = |i| self.logs_dir.join(format!("llm_request.{}.jsonl", i));

            for i in (0..LOGS_TO_KEEP - 1).rev() {
                let _ = fs_err::rename(log_path(i), log_path(i + 1));
            }

            fs_err::rename(&self.temp_path, log_path(0))?;
        }
        Ok(())
    }
}

impl Drop for RequestLog {
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let _ = self.finish();
    }
}
