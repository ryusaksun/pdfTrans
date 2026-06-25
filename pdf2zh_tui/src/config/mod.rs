use std::path::PathBuf;

use anyhow::Result;

use crate::app::App;

/// Path to the default config file.
fn config_path() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Downloads"))
        .join("pdf2zh")
        .join("config.v3.toml")
}

/// Load settings from the TOML config file and apply them to the App state.
/// Returns Ok(true) if config was loaded, Ok(false) if no config file found.
pub fn load_config(app: &mut App) -> Result<bool> {
    let path = config_path();
    if !path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&path)?;
    let table: toml::Table = content.parse()?;

    // [translation] section
    if let Some(translation) = table.get("translation").and_then(|v| v.as_table()) {
        if let Some(lang_in) = translation.get("lang_in").and_then(|v| v.as_str()) {
            if lang_in != "null" {
                if let Some(idx) = app.language_map.values().position(|code| code == lang_in) {
                    app.lang_in_idx = idx;
                }
            }
        }
        if let Some(lang_out) = translation.get("lang_out").and_then(|v| v.as_str()) {
            if lang_out != "null" {
                if let Some(idx) = app.language_map.values().position(|code| code == lang_out) {
                    app.lang_out_idx = idx;
                }
            }
        }
        if let Some(qps) = translation.get("qps").and_then(|v| v.as_integer()) {
            app.qps = qps.max(1) as u32;
            app.qps_input = app.qps.to_string();
        }
        if let Some(prompt) = translation
            .get("custom_system_prompt")
            .and_then(|v| v.as_str())
        {
            if prompt != "null" {
                app.custom_prompt = prompt.to_string();
            }
        }
        if let Some(min_len) = translation
            .get("min_text_length")
            .and_then(|v| v.as_integer())
        {
            app.min_text_length = min_len.to_string();
        }
        if let Some(v) = translation
            .get("pool_max_workers")
            .and_then(|v| v.as_integer())
        {
            app.pool_max_workers_input = v.to_string();
        }
        if let Some(v) = translation.get("term_qps").and_then(|v| v.as_integer()) {
            app.term_qps_input = v.to_string();
        }
        if let Some(v) = translation
            .get("term_pool_max_workers")
            .and_then(|v| v.as_integer())
        {
            app.term_pool_max_workers_input = v.to_string();
        }
        if let Some(v) = translation.get("output").and_then(|v| v.as_str()) {
            if v != "null" {
                app.output_dir = v.to_string();
            }
        }
        if let Some(v) = translation.get("glossaries").and_then(|v| v.as_str()) {
            if v != "null" {
                app.glossary_files = v.to_string();
            }
        }
        if let Some(v) = translation
            .get("save_auto_extracted_glossary")
            .and_then(|v| v.as_bool())
        {
            app.save_auto_extracted_glossary = v;
        }
        if let Some(v) = translation
            .get("no_auto_extract_glossary")
            .and_then(|v| v.as_bool())
        {
            app.disable_auto_extract_glossary = v;
        }
    }

    // [pdf] section
    if let Some(pdf) = table.get("pdf").and_then(|v| v.as_table()) {
        if let Some(v) = pdf.get("no_dual").and_then(|v| v.as_bool()) {
            app.no_dual = v;
        }
        if let Some(v) = pdf.get("no_mono").and_then(|v| v.as_bool()) {
            app.no_mono = v;
        }
        if let Some(v) = pdf.get("dual_translate_first").and_then(|v| v.as_bool()) {
            app.dual_translate_first = v;
        }
        if let Some(v) = pdf.get("skip_clean").and_then(|v| v.as_bool()) {
            app.skip_clean = v;
        }
        if let Some(v) = pdf.get("max_pages_per_part").and_then(|v| v.as_integer()) {
            app.max_pages_per_part_input = v.to_string();
        }
        if let Some(v) = pdf.get("skip_scanned_detection").and_then(|v| v.as_bool()) {
            app.skip_scanned_detection = v;
        }
        if let Some(v) = pdf
            .get("only_include_translated_page")
            .and_then(|v| v.as_bool())
        {
            app.only_include_translated_page = v;
        }
    }

    // Find active engine from top-level flags
    let engine_flags = [
        "google",
        "bing",
        "deepl",
        "openai",
        "zhipu",
        "siliconflow",
        "siliconflowfree",
        "gemini",
        "azure",
        "tencent",
        "dify",
        "anythingllm",
        "ollama",
        "grok",
        "groq",
        "deepseek",
        "qwenmt",
        "openaicompatible",
        "claudecode",
        "aliyundashscope",
        "azureopenai",
        "modelscope",
        "xinference",
    ];
    for flag in &engine_flags {
        if table.get(*flag).and_then(|v| v.as_bool()) == Some(true) {
            if let Some(idx) = app
                .engine_schemas
                .iter()
                .position(|s| s.cli_flag == *flag || s.name.to_lowercase() == *flag)
            {
                app.engine_idx = idx;
                break;
            }
        }
    }

    load_engine_params(app, &table);
    Ok(true)
}

/// Load engine-specific parameters from the TOML config.
fn load_engine_params(app: &mut App, table: &toml::Table) {
    let schema = match app.engine_schemas.get(app.engine_idx) {
        Some(s) => s.clone(),
        None => return,
    };

    let detail_key = format!("{}_detail", schema.cli_flag);
    let detail = match table.get(&detail_key).and_then(|v| v.as_table()) {
        Some(d) => d,
        None => return,
    };

    app.engine_params.clear();
    for field in &schema.fields {
        if let Some(value) = detail.get(&field.name) {
            let str_val = match value {
                toml::Value::String(s) if s != "null" => s.clone(),
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Float(f) => f.to_string(),
                toml::Value::Boolean(b) => b.to_string(),
                _ => continue,
            };
            if !str_val.is_empty() {
                app.engine_params.insert(field.name.clone(), str_val);
            }
        }
    }
}

/// Save current App settings to the TOML config file.
/// Uses toml_edit to preserve existing content and comments.
pub fn save_config(app: &App) -> Result<()> {
    let path = config_path();

    // Ensure config directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Load existing or create new document
    let mut doc: toml_edit::DocumentMut = if path.exists() {
        std::fs::read_to_string(&path)?.parse()?
    } else {
        toml_edit::DocumentMut::new()
    };

    // Reset all engine flags to false first
    for schema in &app.engine_schemas {
        doc[&schema.cli_flag] = toml_edit::value(false);
    }

    // Set active engine flag
    if let Some(schema) = app.engine_schemas.get(app.engine_idx) {
        doc[&schema.cli_flag] = toml_edit::value(true);
    }

    // [translation] section
    let translation = doc["translation"]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .unwrap();

    let lang_in = app
        .language_map
        .get_index(app.lang_in_idx)
        .map(|(_, v)| v.as_str())
        .unwrap_or("en");
    let lang_out = app
        .language_map
        .get_index(app.lang_out_idx)
        .map(|(_, v)| v.as_str())
        .unwrap_or("zh-CN");

    translation["lang_in"] = toml_edit::value(lang_in);
    translation["lang_out"] = toml_edit::value(lang_out);
    translation["qps"] = toml_edit::value(app.qps as i64);

    if app.custom_prompt.is_empty() {
        translation["custom_system_prompt"] = toml_edit::value("null");
    } else {
        translation["custom_system_prompt"] = toml_edit::value(&app.custom_prompt);
    }

    if app.min_text_length.is_empty() {
        translation["min_text_length"] = toml_edit::value(5i64);
    } else if let Ok(n) = app.min_text_length.parse::<i64>() {
        translation["min_text_length"] = toml_edit::value(n);
    }
    if let Ok(n) = app.pool_max_workers_input.parse::<i64>() {
        translation["pool_max_workers"] = toml_edit::value(n);
    } else {
        translation["pool_max_workers"] = toml_edit::value("null");
    }
    if let Ok(n) = app.term_qps_input.parse::<i64>() {
        translation["term_qps"] = toml_edit::value(n);
    } else {
        translation["term_qps"] = toml_edit::value("null");
    }
    if let Ok(n) = app.term_pool_max_workers_input.parse::<i64>() {
        translation["term_pool_max_workers"] = toml_edit::value(n);
    } else {
        translation["term_pool_max_workers"] = toml_edit::value("null");
    }
    if app.output_dir.is_empty() {
        translation["output"] = toml_edit::value("null");
    } else {
        translation["output"] = toml_edit::value(&app.output_dir);
    }
    if app.glossary_files.is_empty() {
        translation["glossaries"] = toml_edit::value("null");
    } else {
        translation["glossaries"] = toml_edit::value(&app.glossary_files);
    }
    translation["save_auto_extracted_glossary"] =
        toml_edit::value(app.save_auto_extracted_glossary);
    translation["no_auto_extract_glossary"] = toml_edit::value(app.disable_auto_extract_glossary);

    // [pdf] section
    let pdf = doc["pdf"]
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .unwrap();

    pdf["no_dual"] = toml_edit::value(app.no_dual);
    pdf["no_mono"] = toml_edit::value(app.no_mono);
    pdf["dual_translate_first"] = toml_edit::value(app.dual_translate_first);
    pdf["skip_clean"] = toml_edit::value(app.skip_clean);
    if let Ok(n) = app.max_pages_per_part_input.parse::<i64>() {
        pdf["max_pages_per_part"] = toml_edit::value(n);
    } else {
        pdf["max_pages_per_part"] = toml_edit::value("null");
    }
    pdf["skip_scanned_detection"] = toml_edit::value(app.skip_scanned_detection);
    pdf["only_include_translated_page"] = toml_edit::value(app.only_include_translated_page);

    // Engine detail section
    if let Some(schema) = app.engine_schemas.get(app.engine_idx) {
        let detail_key = format!("{}_detail", schema.cli_flag);
        let detail = doc[&detail_key]
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .unwrap();

        detail["translate_engine_type"] = toml_edit::value(&schema.name);
        detail["support_llm"] = toml_edit::value(if schema.support_llm { "yes" } else { "no" });

        for field in &schema.fields {
            if let Some(value) = app.engine_params.get(&field.name) {
                if value.is_empty() {
                    detail[&field.name] = toml_edit::value("null");
                } else {
                    match field.field_type.as_str() {
                        "bool" => {
                            detail[&field.name] = toml_edit::value(value == "true");
                        }
                        "int" => {
                            if let Ok(n) = value.parse::<i64>() {
                                detail[&field.name] = toml_edit::value(n);
                            } else {
                                detail[&field.name] = toml_edit::value(value.as_str());
                            }
                        }
                        _ => {
                            detail[&field.name] = toml_edit::value(value.as_str());
                        }
                    }
                }
            } else {
                detail[&field.name] = toml_edit::value("null");
            }
        }
    }

    // Write to file
    std::fs::write(&path, doc.to_string())?;
    Ok(())
}
