use crate::parse_scene;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionObservation {
    pub kind: String,
    pub text: Option<String>,
    pub bbox: VisionBox,
    pub confidence: f64,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    pub font_size: Option<f64>,
    #[serde(default)]
    pub font_weight: Option<String>,
    #[serde(default)]
    pub baseline_y: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionManifest {
    pub schema_version: u32,
    pub source: String,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub background: Option<String>,
    pub observations: Vec<VisionObservation>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VisionImportMode {
    Structured,
    Hybrid,
    Auto,
}

impl std::fmt::Display for VisionImportMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Structured => "structured",
            Self::Hybrid => "hybrid",
            Self::Auto => "auto",
        })
    }
}

impl std::str::FromStr for VisionImportMode {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "structured" => Ok(Self::Structured),
            "hybrid" => Ok(Self::Hybrid),
            "auto" => Ok(Self::Auto),
            _ => Err(format!(
                "unknown Vision import mode {value:?}; expected structured, hybrid, or auto"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VisionImportSummary {
    pub source: String,
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub mode: VisionImportMode,
    pub text_nodes: usize,
    pub region_nodes: usize,
    pub raster_nodes: usize,
    pub semantic_overlays: usize,
}

pub fn default_vision_helper() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("studio/.build/debug/GlyphVision")
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn finite(value: f64, label: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(anyhow!("Vision manifest contains non-finite {label}"))
    }
}

fn validate_manifest(manifest: &VisionManifest) -> Result<()> {
    if manifest.schema_version != 1 {
        return Err(anyhow!(
            "unsupported Vision manifest schema {}",
            manifest.schema_version
        ));
    }
    if manifest.width == 0 || manifest.height == 0 {
        return Err(anyhow!("Vision manifest has invalid image dimensions"));
    }
    let max_x = manifest.width as f64;
    let max_y = manifest.height as f64;
    for (index, observation) in manifest.observations.iter().enumerate() {
        if observation.kind != "text"
            && observation.kind != "rectangle"
            && observation.kind != "ellipse"
        {
            return Err(anyhow!(
                "unsupported Vision observation kind '{}' at index {index}",
                observation.kind
            ));
        }
        if !(0.0..=1.0).contains(&finite(observation.confidence, "confidence")?) {
            return Err(anyhow!("Vision confidence out of range at index {index}"));
        }
        for (value, label) in [
            (observation.bbox.x, "bbox.x"),
            (observation.bbox.y, "bbox.y"),
            (observation.bbox.width, "bbox.width"),
            (observation.bbox.height, "bbox.height"),
        ] {
            finite(value, label)?;
        }
        if observation.bbox.width <= 0.0 || observation.bbox.height <= 0.0 {
            return Err(anyhow!(
                "Vision bbox has non-positive size at index {index}"
            ));
        }
        if observation.bbox.x < -0.5
            || observation.bbox.y < -0.5
            || observation.bbox.x + observation.bbox.width > max_x + 0.5
            || observation.bbox.y + observation.bbox.height > max_y + 0.5
        {
            return Err(anyhow!("Vision bbox escapes canvas at index {index}"));
        }
    }
    Ok(())
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
fn fmt_num(value: f64) -> String {
    let mut out = format!("{value:.3}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    if out == "-0" { "0".into() } else { out }
}

fn safe_color(value: Option<&str>, fallback: &str) -> String {
    let Some(value) = value else {
        return fallback.to_string();
    };
    let valid = value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        value.to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

fn observation_hash(observation: &VisionObservation) -> String {
    let payload = format!(
        "{}|{}|{:.3}|{:.3}|{:.3}|{:.3}",
        observation.kind,
        observation.text.as_deref().unwrap_or(""),
        observation.bbox.x,
        observation.bbox.y,
        observation.bbox.width,
        observation.bbox.height
    );
    format!("{:012x}", fnv1a64(payload.as_bytes()) & 0xffffffffffff)
}

fn scene_slug(source: &str) -> String {
    let stem = Path::new(source)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let mut slug = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while slug.contains("__") {
        slug = slug.replace("__", "_");
    }
    slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        slug = "image".into();
    }
    slug
}
fn unique_id(observation: &VisionObservation, seen: &mut BTreeMap<String, usize>) -> String {
    let prefix = match observation.kind.as_str() {
        "text" => "text",
        "ellipse" => "ellipse",
        _ => "region",
    };
    let base = format!("{}_{}", prefix, observation_hash(observation));
    let count = seen.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{}_{}", base, count)
    }
}

fn box_contains(container: &VisionBox, inner: &VisionBox) -> bool {
    const EPS: f64 = 2.0;
    inner.x >= container.x - EPS
        && inner.y >= container.y - EPS
        && inner.x + inner.width <= container.x + container.width + EPS
        && inner.y + inner.height <= container.y + container.height + EPS
}

fn inferred_text_anchor(
    observation: &VisionObservation,
    observations: &[VisionObservation],
) -> (f64, &'static str) {
    let text_center = observation.bbox.x + observation.bbox.width / 2.0;
    let containing = observations
        .iter()
        .filter(|candidate| {
            candidate.kind != "text" && box_contains(&candidate.bbox, &observation.bbox)
        })
        .min_by(|a, b| {
            let area_a = a.bbox.width * a.bbox.height;
            let area_b = b.bbox.width * b.bbox.height;
            area_a.total_cmp(&area_b)
        });
    if let Some(container) = containing {
        let center = container.bbox.x + container.bbox.width / 2.0;
        let tolerance = (container.bbox.width * 0.05).max(4.0);
        if (text_center - center).abs() <= tolerance {
            return (center, "center");
        }
    }
    (observation.bbox.x, "start")
}

pub fn choose_import_mode(manifest: &VisionManifest) -> Result<VisionImportMode> {
    validate_manifest(manifest)?;
    if manifest.observations.is_empty() {
        return Ok(VisionImportMode::Hybrid);
    }
    let region_count = manifest
        .observations
        .iter()
        .filter(|observation| observation.kind != "text")
        .count();
    let text_count = manifest
        .observations
        .iter()
        .filter(|observation| observation.kind == "text")
        .count();
    let mean_confidence = manifest
        .observations
        .iter()
        .map(|observation| observation.confidence)
        .sum::<f64>()
        / manifest.observations.len() as f64;
    let structured = mean_confidence >= 0.78
        && (manifest.observations.len() >= 4 || (region_count >= 1 && text_count >= 2));
    Ok(if structured {
        VisionImportMode::Structured
    } else {
        VisionImportMode::Hybrid
    })
}

pub fn manifest_to_glyph(manifest: &VisionManifest) -> Result<String> {
    validate_manifest(manifest)?;
    let slug = scene_slug(&manifest.source);
    let filename = Path::new(&manifest.source)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let mut observations = manifest.observations.clone();
    observations.sort_by(|a, b| {
        a.bbox
            .y
            .total_cmp(&b.bbox.y)
            .then(a.bbox.x.total_cmp(&b.bbox.x))
            .then(a.kind.cmp(&b.kind))
            .then(a.text.cmp(&b.text))
    });
    let mut out = String::new();
    out.push_str(&format!(
        "scene \"Vision Import: {}\" @id(\"vision_{}\") {{\n",
        escape_string(filename),
        slug
    ));
    let background = safe_color(manifest.background.as_deref(), "#ffffff");
    out.push_str(&format!(
        "  canvas {} {} background \"{}\";\n",
        manifest.width, manifest.height, background
    ));
    out.push_str(&format!(
        "  meta \"vision.source\" \"{}\";\n",
        escape_string(&manifest.source)
    ));
    out.push_str("  meta \"vision.importer\" \"apple-vision-v1\";\n");
    out.push_str("  meta \"vision.reconstruction\" \"bounded-ocr-and-rectangles\";\n\n");
    let mut seen = BTreeMap::new();
    for observation in &observations {
        let id = unique_id(observation, &mut seen);
        let confidence = observation.confidence.clamp(0.0, 1.0);
        if observation.kind == "text" {
            let text = observation.text.as_deref().unwrap_or("");
            if text.is_empty() {
                continue;
            }
            let size = observation
                .font_size
                .unwrap_or(observation.bbox.height * 0.78)
                .clamp(8.0, 512.0);
            let baseline = observation
                .baseline_y
                .unwrap_or(observation.bbox.y + observation.bbox.height * 0.82);
            let weight = observation.font_weight.as_deref().unwrap_or("400");
            let (anchor_x, align) = inferred_text_anchor(observation, &observations);
            let text_color = safe_color(observation.color.as_deref(), "#111111");
            out.push_str(&format!("  text {id} {{\n"));
            out.push_str("    role \"vision\" \"inferred\" \"text\";\n");
            out.push_str(&format!(
                "    confidence semantics {} geometry {} depth unknown;\n",
                fmt_num(confidence),
                fmt_num(confidence)
            ));
            out.push_str(&format!(
                "    x {}; y {};\n",
                fmt_num(anchor_x),
                fmt_num(baseline)
            ));
            out.push_str(&format!(
                "    content \"{}\"; size {};\n",
                escape_string(text),
                fmt_num(size)
            ));
            out.push_str(&format!(
                "    font \"system-ui\"; weight \"{}\"; align \"{}\"; fill \"{}\";\n",
                escape_string(weight),
                align,
                text_color
            ));
            out.push_str("  }\n\n");
        } else {
            let semantic_confidence = (confidence * 0.75).min(0.75);
            let fill = safe_color(observation.color.as_deref(), "#e5e7eb");
            let min_dimension = observation.bbox.width.min(observation.bbox.height);
            let corner_radius = observation
                .corner_radius
                .unwrap_or(0.0)
                .clamp(0.0, min_dimension / 2.0);
            let aspect = observation.bbox.width / observation.bbox.height;
            let ellipse_like = observation.kind == "ellipse"
                || (corner_radius >= min_dimension * 0.40 && (0.75..=1.33).contains(&aspect));
            if ellipse_like {
                out.push_str(&format!("  ellipse {id} {{\n"));
            } else {
                out.push_str(&format!("  rect {id} {{\n"));
            }
            out.push_str("    role \"vision\" \"inferred\" \"region\";\n");
            out.push_str(&format!(
                "    confidence semantics {} geometry {} depth unknown;\n",
                fmt_num(semantic_confidence),
                fmt_num(confidence)
            ));
            if ellipse_like {
                out.push_str(&format!(
                    "    cx {}; cy {}; rx {}; ry {};\n",
                    fmt_num(observation.bbox.x + observation.bbox.width / 2.0),
                    fmt_num(observation.bbox.y + observation.bbox.height / 2.0),
                    fmt_num(observation.bbox.width / 2.0),
                    fmt_num(observation.bbox.height / 2.0)
                ));
            } else {
                out.push_str(&format!(
                    "    x {}; y {}; width {}; height {}; rx {};\n",
                    fmt_num(observation.bbox.x),
                    fmt_num(observation.bbox.y),
                    fmt_num(observation.bbox.width),
                    fmt_num(observation.bbox.height),
                    fmt_num(corner_radius)
                ));
            }
            out.push_str(&format!("    fill \"{}\";\n", fill));
            out.push_str("  }\n\n");
        }
    }
    out.push_str("}\n");
    parse_scene(&out, "<vision-import>")
        .context("generated Vision import did not parse as Glyph")?;
    Ok(out)
}

pub fn manifest_to_hybrid_glyph(manifest: &VisionManifest) -> Result<String> {
    validate_manifest(manifest)?;
    let mut out = manifest_to_glyph(manifest)?;
    let parsed = parse_scene(&out, "<vision-hybrid-base>")?;
    for node in parsed.nodes {
        out = crate::source_edit::set_source_property(
            &out,
            &node.id,
            "opacity",
            crate::model::Value::Number(0.0),
        )?;
        out = crate::source_edit::set_source_property(
            &out,
            &node.id,
            "a11y_hidden",
            crate::model::Value::Text("true".into()),
        )?;
    }
    out = out.replace(
        "meta \"vision.reconstruction\" \"bounded-ocr-and-rectangles\";",
        "meta \"vision.reconstruction\" \"hybrid-raster-semantic-overlays\";\n  meta \"vision.hybrid.overlay_policy\" \"non-rendering-confidence-bearing\";",
    );
    let close = out
        .rfind('}')
        .ok_or_else(|| anyhow!("generated Vision source has no scene closing brace"))?;
    let raster = format!(
        "  image vision_raster_base @id(\"vision_raster_base\") {{\n    role \"vision\" \"hybrid-raster\" \"reference\";\n    confidence semantics 0.1 geometry 1 depth unknown;\n    x 0; y 0; width {}; height {};\n    src \"{}\"; fit \"fill\";\n    a11y_hidden \"true\";\n  }}\n\n",
        manifest.width,
        manifest.height,
        escape_string(&manifest.source)
    );
    out.insert_str(close, &raster);
    parse_scene(&out, "<vision-hybrid-import>")
        .context("generated hybrid Vision import did not parse as Glyph")?;
    Ok(out)
}

pub fn source_for_manifest(
    manifest: &VisionManifest,
    requested_mode: VisionImportMode,
) -> Result<(String, VisionImportMode)> {
    let mode = match requested_mode {
        VisionImportMode::Auto => choose_import_mode(manifest)?,
        explicit => explicit,
    };
    let source = match mode {
        VisionImportMode::Structured => manifest_to_glyph(manifest)?,
        VisionImportMode::Hybrid => manifest_to_hybrid_glyph(manifest)?,
        VisionImportMode::Auto => unreachable!("auto mode resolved above"),
    };
    Ok((source, mode))
}

pub fn run_vision_helper(image: &Path, helper: &Path) -> Result<VisionManifest> {
    let output = Command::new(helper)
        .arg(image)
        .output()
        .with_context(|| format!("launch GlyphVision helper {}", helper.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("GlyphVision failed: {}", stderr.trim()));
    }
    let manifest: VisionManifest =
        serde_json::from_slice(&output.stdout).context("decode GlyphVision JSON manifest")?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn import_image_to_glyph_with_mode(
    image: &Path,
    output: &Path,
    helper: &Path,
    manifest_output: Option<&Path>,
    requested_mode: VisionImportMode,
) -> Result<VisionImportSummary> {
    let manifest = run_vision_helper(image, helper)?;
    let (source, mode) = source_for_manifest(&manifest, requested_mode)?;
    fs::write(output, source)
        .with_context(|| format!("write imported Glyph source {}", output.display()))?;
    if let Some(path) = manifest_output {
        fs::write(path, serde_json::to_string_pretty(&manifest)? + "\n")
            .with_context(|| format!("write Vision manifest {}", path.display()))?;
    }
    let text_nodes = manifest
        .observations
        .iter()
        .filter(|o| o.kind == "text")
        .count();
    let region_nodes = manifest
        .observations
        .iter()
        .filter(|o| o.kind != "text")
        .count();
    let hybrid = mode == VisionImportMode::Hybrid;
    Ok(VisionImportSummary {
        source: manifest.source,
        output: output.display().to_string(),
        width: manifest.width,
        height: manifest.height,
        mode,
        text_nodes,
        region_nodes,
        raster_nodes: usize::from(hybrid),
        semantic_overlays: if hybrid { text_nodes + region_nodes } else { 0 },
    })
}

pub fn import_image_to_glyph(
    image: &Path,
    output: &Path,
    helper: &Path,
    manifest_output: Option<&Path>,
) -> Result<VisionImportSummary> {
    import_image_to_glyph_with_mode(
        image,
        output,
        helper,
        manifest_output,
        VisionImportMode::Auto,
    )
}
