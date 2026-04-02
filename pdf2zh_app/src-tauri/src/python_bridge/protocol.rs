use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Events received from the Python subprocess via stdout (JSON Lines).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum PythonEvent {
    #[serde(rename = "ready")]
    Ready {
        version: String,
        engines: Vec<String>,
    },

    #[serde(rename = "config_schema")]
    ConfigSchema {
        engines: Vec<EngineSchema>,
        languages: indexmap::IndexMap<String, String>,
    },

    #[serde(rename = "stage_summary")]
    StageSummary {
        stages: Vec<StageDef>,
        part_index: u32,
        total_parts: u32,
    },

    #[serde(rename = "progress_start")]
    ProgressStart(ProgressData),

    #[serde(rename = "progress_update")]
    ProgressUpdate(ProgressData),

    #[serde(rename = "progress_end")]
    ProgressEnd(ProgressData),

    #[serde(rename = "finish")]
    Finish {
        translate_result: serde_json::Value,
        token_usage: Option<HashMap<String, TokenUsage>>,
    },

    #[serde(rename = "error")]
    Error {
        error: String,
        error_type: String,
        #[serde(default)]
        details: String,
    },

    #[serde(rename = "validation_result")]
    ValidationResult {
        valid: bool,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StageDef {
    pub name: String,
    #[serde(default)]
    pub percent: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProgressData {
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub stage_progress: f64,
    #[serde(default)]
    pub stage_current: u32,
    #[serde(default)]
    pub stage_total: u32,
    #[serde(default)]
    pub overall_progress: f64,
    #[serde(default)]
    pub part_index: u32,
    #[serde(default)]
    pub total_parts: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub prompt: i64,
    #[serde(default)]
    pub completion: i64,
    #[serde(default)]
    pub cache_hit_prompt: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineSchema {
    pub name: String,
    #[serde(default)]
    pub support_llm: bool,
    #[serde(default)]
    pub cli_flag: String,
    #[serde(default)]
    pub fields: Vec<FieldSchema>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldSchema {
    pub name: String,
    #[serde(rename = "type", default)]
    pub field_type: String,
    #[serde(default)]
    pub default: serde_json::Value,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sensitive: bool,
    #[serde(default)]
    pub password: bool,
}

/// Commands sent to the Python subprocess via stdin (JSON Lines).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "cmd")]
pub enum PythonCommand {
    #[serde(rename = "translate")]
    Translate {
        settings: serde_json::Value,
        files: Vec<String>,
    },

    #[serde(rename = "cancel")]
    Cancel,

    #[serde(rename = "validate")]
    Validate { settings: serde_json::Value },

    #[serde(rename = "shutdown")]
    Shutdown,
}
