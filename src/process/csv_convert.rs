
use anyhow::Result;
use csv::Reader;
use serde_json::{Map, Value};
use std::fs;
use crate::cli::OutputFormat;

pub fn process_csv(input: &str, output: String, format: OutputFormat) -> Result<()> {
    let mut reader = Reader::from_path(input)?;
    let mut ret: Vec<Value> = Vec::with_capacity(128);
    let headers = reader.headers()?.clone();

    for result in reader.records() {
        let record = result?;
        let json_value = headers
            .iter()
            .zip(record.iter())
            .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
            .collect::<Map<String, Value>>();
        ret.push(Value::Object(json_value));
    }
    
    let content = match format {
        OutputFormat::Json => serde_json::to_string_pretty(&ret)?,
        OutputFormat::Yaml => serde_yaml::to_string(&ret)?,
        // TOML 文档的根必须是表(table)，不能是数组，
        // 直接序列化 Vec<Value> 会报 `unsupported array type`，
        // 所以这里把整个数组挂到 rows 键下，生成 [[rows]] 数组表
        OutputFormat::Toml => toml::to_string(&serde_json::json!({ "rows": ret }))?,
    };

    fs::write(output, content)?;
    
    Ok(())
}