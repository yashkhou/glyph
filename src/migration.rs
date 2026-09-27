use crate::model::{CURRENT_GSIR_SCHEMA_VERSION, CURRENT_SOURCE_VERSION, Scene};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationReport {
    pub from_schema: u32,
    pub to_schema: u32,
    pub migrated: bool,
    pub steps: Vec<String>,
}

fn schema_version(value: &JsonValue) -> Result<u32> {
    let raw = value
        .get("schema_version")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| anyhow!("GSIR is missing integer schema_version"))?;
    u32::try_from(raw).map_err(|_| anyhow!("GSIR schema_version {raw} is out of range"))
}

pub fn migrate_gsir_value(mut value: JsonValue) -> Result<(JsonValue, MigrationReport)> {
    let from_schema = schema_version(&value)?;
    if from_schema == 0 || from_schema > CURRENT_GSIR_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported GSIR schema version {from_schema}; current version is {CURRENT_GSIR_SCHEMA_VERSION}"
        ));
    }

    let mut steps = Vec::new();
    if from_schema == 1 {
        let object = value
            .as_object_mut()
            .ok_or_else(|| anyhow!("GSIR root must be a JSON object"))?;
        object.insert(
            "schema_version".into(),
            JsonValue::from(CURRENT_GSIR_SCHEMA_VERSION),
        );
        object
            .entry("source_version")
            .or_insert_with(|| JsonValue::from(CURRENT_SOURCE_VERSION));
        steps.push("gsir-v1-to-v2".into());
    }

    let to_schema = schema_version(&value)?;
    Ok((
        value,
        MigrationReport {
            from_schema,
            to_schema,
            migrated: from_schema != to_schema,
            steps,
        },
    ))
}

pub fn load_gsir_str(input: &str) -> Result<(Scene, MigrationReport)> {
    let value: JsonValue = serde_json::from_str(input).context("invalid GSIR JSON")?;
    let (value, report) = migrate_gsir_value(value)?;
    let scene: Scene = serde_json::from_value(value).context("invalid GSIR scene")?;
    if scene.schema_version != CURRENT_GSIR_SCHEMA_VERSION {
        return Err(anyhow!("GSIR migration did not reach current schema"));
    }
    if scene.source_version == 0 || scene.source_version > CURRENT_SOURCE_VERSION {
        return Err(anyhow!(
            "unsupported Glyph source version {} in GSIR; current version is {}",
            scene.source_version,
            CURRENT_SOURCE_VERSION
        ));
    }
    Ok((scene, report))
}
