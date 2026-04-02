use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Saved user preferences (compatible with TUI config format).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub engine_name: String,
    pub engine_cli_flag: String,
    pub lang_in: String,
    pub lang_out: String,
    pub qps: u32,
    pub custom_prompt: String,
    pub no_dual: bool,
    pub no_mono: bool,
    pub dual_translate_first: bool,
    pub skip_clean: bool,
    pub engine_params: std::collections::HashMap<String, String>,
}

fn config_path() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"))
        .join("pdf2zh")
        .join("config.v3.toml")
}

/// Engine flags list (same as TUI).
const ENGINE_FLAGS: &[&str] = &[
    "google", "bing", "deepl", "openai", "zhipu", "siliconflow", "siliconflowfree",
    "gemini", "azure", "tencent", "dify", "anythingllm", "ollama", "grok", "groq",
    "deepseek", "qwenmt", "openaicompatible", "claudecode", "aliyundashscope",
    "azureopenai", "modelscope", "xinference",
];

pub fn load_config() -> Result<AppConfig> {
    let path = config_path();
    if !path.exists() {
        return Ok(AppConfig {
            qps: 4,
            lang_in: "en".into(),
            lang_out: "zh-CN".into(),
            ..Default::default()
        });
    }

    let content = std::fs::read_to_string(&path)?;
    let table: toml::Table = content.parse()?;
    let mut cfg = AppConfig {
        qps: 4,
        lang_in: "en".into(),
        lang_out: "zh-CN".into(),
        ..Default::default()
    };

    // Translation section
    if let Some(translation) = table.get("translation").and_then(|v| v.as_table()) {
        if let Some(v) = translation.get("lang_in").and_then(|v| v.as_str()) {
            if v != "null" { cfg.lang_in = v.to_string(); }
        }
        if let Some(v) = translation.get("lang_out").and_then(|v| v.as_str()) {
            if v != "null" { cfg.lang_out = v.to_string(); }
        }
        if let Some(v) = translation.get("qps").and_then(|v| v.as_integer()) {
            cfg.qps = v.max(1) as u32;
        }
        if let Some(v) = translation.get("custom_system_prompt").and_then(|v| v.as_str()) {
            if v != "null" { cfg.custom_prompt = v.to_string(); }
        }
    }

    // PDF section
    if let Some(pdf) = table.get("pdf").and_then(|v| v.as_table()) {
        cfg.no_dual = pdf.get("no_dual").and_then(|v| v.as_bool()).unwrap_or(false);
        cfg.no_mono = pdf.get("no_mono").and_then(|v| v.as_bool()).unwrap_or(false);
        cfg.dual_translate_first = pdf.get("dual_translate_first").and_then(|v| v.as_bool()).unwrap_or(false);
        cfg.skip_clean = pdf.get("skip_clean").and_then(|v| v.as_bool()).unwrap_or(false);
    }

    // Find active engine
    for flag in ENGINE_FLAGS {
        if table.get(*flag).and_then(|v| v.as_bool()) == Some(true) {
            cfg.engine_cli_flag = flag.to_string();

            // Load engine detail params
            let detail_key = format!("{flag}_detail");
            if let Some(detail) = table.get(&detail_key).and_then(|v| v.as_table()) {
                for (k, v) in detail {
                    if k == "translate_engine_type" || k == "support_llm" { continue; }
                    let str_val = match v {
                        toml::Value::String(s) if s != "null" => s.clone(),
                        toml::Value::Integer(i) => i.to_string(),
                        toml::Value::Float(f) => f.to_string(),
                        toml::Value::Boolean(b) => b.to_string(),
                        _ => continue,
                    };
                    cfg.engine_params.insert(k.clone(), str_val);
                }
                // Get engine name from detail
                if let Some(name) = detail.get("translate_engine_type").and_then(|v| v.as_str()) {
                    cfg.engine_name = name.to_string();
                }
            }
            break;
        }
    }

    Ok(cfg)
}

pub fn save_config(cfg: &AppConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut doc: toml_edit::DocumentMut = if path.exists() {
        std::fs::read_to_string(&path)?.parse()?
    } else {
        toml_edit::DocumentMut::new()
    };

    // Reset all engine flags
    for flag in ENGINE_FLAGS {
        doc[*flag] = toml_edit::value(false);
    }
    // Set active engine
    if !cfg.engine_cli_flag.is_empty() {
        doc[&cfg.engine_cli_flag] = toml_edit::value(true);
    }

    // Translation
    let translation = doc["translation"]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut().unwrap();
    translation["lang_in"] = toml_edit::value(&cfg.lang_in);
    translation["lang_out"] = toml_edit::value(&cfg.lang_out);
    translation["qps"] = toml_edit::value(cfg.qps as i64);
    translation["custom_system_prompt"] = toml_edit::value(
        if cfg.custom_prompt.is_empty() { "null" } else { &cfg.custom_prompt }
    );

    // PDF
    let pdf = doc["pdf"]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut().unwrap();
    pdf["no_dual"] = toml_edit::value(cfg.no_dual);
    pdf["no_mono"] = toml_edit::value(cfg.no_mono);
    pdf["dual_translate_first"] = toml_edit::value(cfg.dual_translate_first);
    pdf["skip_clean"] = toml_edit::value(cfg.skip_clean);

    // Engine detail
    if !cfg.engine_cli_flag.is_empty() {
        let detail_key = format!("{}_detail", cfg.engine_cli_flag);
        let detail = doc[&detail_key]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut().unwrap();
        if !cfg.engine_name.is_empty() {
            detail["translate_engine_type"] = toml_edit::value(&cfg.engine_name);
        }
        for (k, v) in &cfg.engine_params {
            detail[k.as_str()] = toml_edit::value(
                if v.is_empty() { "null" } else { v.as_str() }
            );
        }
    }

    std::fs::write(&path, doc.to_string())?;
    Ok(())
}
