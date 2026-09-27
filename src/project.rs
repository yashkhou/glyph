use crate::{
    model::{ComponentDefinition, ImportSpec, Paint, Scene, StateOverride, Value},
    parser::{parse_import_specs, parse_scene_with_environment},
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const PROJECT_MANIFEST: &str = "glyph.project.json";
const PROJECT_MANIFEST_VERSION: u32 = 1;
const PROJECT_BUNDLE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub version: u32,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub assets: BTreeMap<String, ProjectAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAsset {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectFileRecord {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectAssetRecord {
    pub name: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectValidationReport {
    pub root: String,
    pub entry: String,
    pub manifest: Option<String>,
    pub sources: Vec<ProjectFileRecord>,
    pub assets: Vec<ProjectAssetRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectBundleManifest {
    pub version: u32,
    pub root: String,
    pub entry: String,
    pub sources: Vec<ProjectFileRecord>,
    pub assets: Vec<ProjectAssetRecord>,
}

fn read_manifest(path: &Path) -> Result<ProjectManifest> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("read project manifest {}", path.display()))?;
    let manifest: ProjectManifest = serde_json::from_str(&source)
        .with_context(|| format!("parse project manifest {}", path.display()))?;
    if manifest.version != PROJECT_MANIFEST_VERSION {
        return Err(anyhow!(
            "unsupported Glyph project manifest version {}; current version is {PROJECT_MANIFEST_VERSION}",
            manifest.version
        ));
    }
    Ok(manifest)
}

fn validate_manifest(path: &Path) -> Result<()> {
    read_manifest(path).map(|_| ())
}

pub fn discover_project_root(entry: &Path) -> Result<PathBuf> {
    let entry = entry
        .canonicalize()
        .with_context(|| format!("resolve entry {}", entry.display()))?;
    let parent = entry
        .parent()
        .ok_or_else(|| anyhow!("entry {} has no parent directory", entry.display()))?;
    for dir in parent.ancestors() {
        let manifest = dir.join(PROJECT_MANIFEST);
        if manifest.is_file() {
            validate_manifest(&manifest)?;
            return dir
                .canonicalize()
                .with_context(|| format!("resolve project root {}", dir.display()));
        }
    }
    parent
        .canonicalize()
        .with_context(|| format!("resolve project root {}", parent.display()))
}

fn project_relative(path: &Path, root: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| {
        anyhow!(
            "path {} escapes Glyph project root {}",
            path.display(),
            root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn resolve_project_file(root: &Path, requested: &str, kind: &str) -> Result<PathBuf> {
    let relative = Path::new(requested);
    if relative.is_absolute() {
        return Err(anyhow!(
            "absolute {kind} paths are not allowed: {requested}"
        ));
    }
    let path = root.join(relative).canonicalize().with_context(|| {
        format!(
            "resolve declared {kind} {requested} under {}",
            root.display()
        )
    })?;
    if !path.starts_with(root) {
        return Err(anyhow!(
            "declared {kind} {requested} escapes Glyph project root {}",
            root.display()
        ));
    }
    if !path.is_file() {
        return Err(anyhow!("declared {kind} is not a file: {}", path.display()));
    }
    Ok(path)
}

fn resolve_import(importer: &Path, import: &ImportSpec, root: &Path) -> Result<PathBuf> {
    let requested = Path::new(&import.path);
    if requested.is_absolute() {
        return Err(anyhow!(
            "absolute Glyph imports are not allowed: {}",
            import.path
        ));
    }
    if requested.extension().and_then(|x| x.to_str()) != Some("glyph") {
        return Err(anyhow!(
            "Glyph imports must target .glyph files: {}",
            import.path
        ));
    }
    let importer_dir = importer
        .parent()
        .ok_or_else(|| anyhow!("importer {} has no parent directory", importer.display()))?;
    let target = importer_dir.join(requested);
    let target = target
        .canonicalize()
        .with_context(|| format!("resolve import {} from {}", import.path, importer.display()))?;
    if !target.starts_with(root) {
        return Err(anyhow!(
            "import {} escapes Glyph project root {}",
            import.path,
            root.display()
        ));
    }
    if !target.is_file() {
        return Err(anyhow!("Glyph import is not a file: {}", target.display()));
    }
    Ok(target)
}

fn prefix_name(alias: &str, name: &str) -> String {
    format!("{alias}/{name}")
}

fn qualify_paint_value(alias: &str, property: &str, value: &mut Value) {
    if !matches!(property, "fill_paint" | "stroke_paint") {
        return;
    }
    if let Value::Text(name) = value {
        *name = prefix_name(alias, name);
    }
}

fn qualify_override(alias: &str, value: &mut StateOverride) {
    qualify_paint_value(alias, &value.target.property, &mut value.value);
}

fn qualify_exports(
    scene: &Scene,
    alias: &str,
) -> (
    BTreeMap<String, BTreeMap<String, Value>>,
    BTreeMap<String, Paint>,
    Vec<ComponentDefinition>,
) {
    let styles = scene
        .styles
        .iter()
        .map(|(name, values)| {
            let mut values = values.clone();
            for (property, value) in &mut values {
                qualify_paint_value(alias, property, value);
            }
            (prefix_name(alias, name), values)
        })
        .collect();
    let paints = scene
        .paints
        .iter()
        .map(|(name, paint)| (prefix_name(alias, name), paint.clone()))
        .collect();
    let components = scene
        .components
        .iter()
        .cloned()
        .map(|mut component| {
            component.name = prefix_name(alias, &component.name);
            for node in &mut component.nodes {
                for (property, value) in &mut node.authored.props {
                    qualify_paint_value(alias, property, value);
                }
            }
            for binding in component
                .parameters
                .iter_mut()
                .chain(component.slots.iter_mut())
            {
                if let Some(default) = &mut binding.default {
                    qualify_paint_value(alias, &binding.target.property, default);
                }
            }
            for variant in &mut component.variants {
                for value in &mut variant.overrides {
                    qualify_override(alias, value);
                }
            }
            component
        })
        .collect();
    (styles, paints, components)
}

fn merge_exports(
    styles: &mut BTreeMap<String, BTreeMap<String, Value>>,
    paints: &mut BTreeMap<String, Paint>,
    components: &mut Vec<ComponentDefinition>,
    next_styles: BTreeMap<String, BTreeMap<String, Value>>,
    next_paints: BTreeMap<String, Paint>,
    next_components: Vec<ComponentDefinition>,
) -> Result<()> {
    for (name, values) in next_styles {
        if styles.insert(name.clone(), values).is_some() {
            return Err(anyhow!("duplicate imported style namespace {name}"));
        }
    }
    for (name, paint) in next_paints {
        if paints.insert(name.clone(), paint).is_some() {
            return Err(anyhow!("duplicate imported paint namespace {name}"));
        }
    }
    for component in next_components {
        if components
            .iter()
            .any(|existing| existing.name == component.name)
        {
            return Err(anyhow!(
                "duplicate imported component namespace {}",
                component.name
            ));
        }
        components.push(component);
    }
    Ok(())
}

fn load_recursive<'a>(
    path: &Path,
    root: &Path,
    stack: &mut Vec<PathBuf>,
    overlay: Option<(&'a Path, &'a str)>,
) -> Result<Scene> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve Glyph source {}", path.display()))?;
    if !path.starts_with(root) {
        return Err(anyhow!(
            "source {} escapes Glyph project root {}",
            path.display(),
            root.display()
        ));
    }
    if let Some(pos) = stack.iter().position(|item| item == &path) {
        let mut cycle = stack[pos..]
            .iter()
            .map(|item| project_relative(item, root))
            .collect::<Result<Vec<_>>>()?;
        cycle.push(project_relative(&path, root)?);
        return Err(anyhow!(
            "Glyph import cycle detected: {}",
            cycle.join(" -> ")
        ));
    }
    stack.push(path.clone());
    let source = if let Some((overlay_path, overlay_source)) = overlay
        && overlay_path == path
    {
        overlay_source.to_string()
    } else {
        fs::read_to_string(&path)
            .with_context(|| format!("read Glyph source {}", path.display()))?
    };
    let imports = parse_import_specs(&source)?;
    let mut seeded_styles = BTreeMap::new();
    let mut seeded_paints = BTreeMap::new();
    let mut seeded_components = Vec::new();
    for import in &imports {
        let target = resolve_import(&path, import, root)?;
        let imported_scene = load_recursive(&target, root, stack, overlay)?;
        let (styles, paints, components) = qualify_exports(&imported_scene, &import.alias);
        merge_exports(
            &mut seeded_styles,
            &mut seeded_paints,
            &mut seeded_components,
            styles,
            paints,
            components,
        )?;
    }
    let source_name = project_relative(&path, root)?;
    let result = parse_scene_with_environment(
        &source,
        &source_name,
        seeded_styles,
        seeded_paints,
        seeded_components,
        imports,
    );
    stack.pop();
    result
}

pub fn load_project_scene(entry: &Path) -> Result<Scene> {
    let root = discover_project_root(entry)?;
    let entry = entry
        .canonicalize()
        .with_context(|| format!("resolve entry {}", entry.display()))?;
    load_recursive(&entry, &root, &mut Vec::new(), None)
}

pub fn load_project_scene_with_source(entry: &Path, source: &str) -> Result<Scene> {
    let root = discover_project_root(entry)?;
    let entry = entry
        .canonicalize()
        .with_context(|| format!("resolve entry {}", entry.display()))?;
    load_recursive(&entry, &root, &mut Vec::new(), Some((&entry, source)))
}

fn collect_project_sources(
    path: &Path,
    root: &Path,
    seen: &mut BTreeSet<PathBuf>,
    records: &mut Vec<ProjectFileRecord>,
) -> Result<()> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve source {}", path.display()))?;
    if !path.starts_with(root) {
        return Err(anyhow!("source {} escapes project root", path.display()));
    }
    if !seen.insert(path.clone()) {
        return Ok(());
    }
    let source = fs::read_to_string(&path)
        .with_context(|| format!("read Glyph source {}", path.display()))?;
    records.push(ProjectFileRecord {
        path: project_relative(&path, root)?,
        sha256: sha256_file(&path)?,
    });
    for import in parse_import_specs(&source)? {
        let target = resolve_import(&path, &import, root)?;
        collect_project_sources(&target, root, seen, records)?;
    }
    Ok(())
}

pub fn validate_project(entry: &Path) -> Result<ProjectValidationReport> {
    let root = discover_project_root(entry)?;
    let entry = entry
        .canonicalize()
        .with_context(|| format!("resolve entry {}", entry.display()))?;
    load_project_scene(&entry)?;

    let manifest_path = root.join(PROJECT_MANIFEST);
    let manifest = if manifest_path.is_file() {
        Some(read_manifest(&manifest_path)?)
    } else {
        None
    };
    if let Some(manifest) = &manifest
        && let Some(declared_entry) = &manifest.entry
    {
        let resolved = resolve_project_file(&root, declared_entry, "entry")?;
        if resolved != entry {
            return Err(anyhow!(
                "project entry mismatch: manifest declares {}, command resolved {}",
                declared_entry,
                project_relative(&entry, &root)?
            ));
        }
    }

    let mut sources = Vec::new();
    collect_project_sources(&entry, &root, &mut BTreeSet::new(), &mut sources)?;
    sources.sort_by(|a, b| a.path.cmp(&b.path));

    let mut assets = Vec::new();
    if let Some(manifest) = &manifest {
        for (name, asset) in &manifest.assets {
            if asset.sha256.len() != 64
                || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(anyhow!("asset {name} has invalid sha256 digest"));
            }
            let path = resolve_project_file(&root, &asset.path, "asset")?;
            let actual = sha256_file(&path)?;
            if !actual.eq_ignore_ascii_case(&asset.sha256) {
                return Err(anyhow!(
                    "asset {name} hash mismatch: expected {}, got {actual}",
                    asset.sha256
                ));
            }
            assets.push(ProjectAssetRecord {
                name: name.clone(),
                path: project_relative(&path, &root)?,
                sha256: actual,
            });
        }
    }
    assets.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(ProjectValidationReport {
        root: root.display().to_string(),
        entry: project_relative(&entry, &root)?,
        manifest: manifest.map(|_| PROJECT_MANIFEST.to_string()),
        sources,
        assets,
    })
}

pub fn project_bundle_manifest(entry: &Path) -> Result<ProjectBundleManifest> {
    let report = validate_project(entry)?;
    Ok(ProjectBundleManifest {
        version: PROJECT_BUNDLE_VERSION,
        root: ".".into(),
        entry: report.entry,
        sources: report.sources,
        assets: report.assets,
    })
}

fn copy_bundle_file(root: &Path, bundle_root: &Path, relative: &str) -> Result<()> {
    let source = root
        .join(relative)
        .canonicalize()
        .with_context(|| format!("resolve bundle source {relative} under {}", root.display()))?;
    if !source.starts_with(root) {
        return Err(anyhow!(
            "bundle source {relative} escapes Glyph project root"
        ));
    }
    let destination = bundle_root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create bundle directory {}", parent.display()))?;
    }
    fs::copy(&source, &destination).with_context(|| {
        format!(
            "copy bundle file {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

pub fn write_project_bundle_manifest(entry: &Path, output: &Path) -> Result<ProjectBundleManifest> {
    let bundle = project_bundle_manifest(entry)?;
    let bytes = serde_json::to_vec_pretty(&bundle)?;
    fs::write(output, [bytes.as_slice(), b"\n"].concat())
        .with_context(|| format!("write project bundle manifest {}", output.display()))?;
    Ok(bundle)
}

pub fn write_project_bundle(entry: &Path, output: &Path) -> Result<ProjectBundleManifest> {
    if output.exists() {
        return Err(anyhow!(
            "project bundle output already exists: {}",
            output.display()
        ));
    }
    let root = discover_project_root(entry)?;
    let bundle = project_bundle_manifest(entry)?;
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create bundle parent {}", parent.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| anyhow!("read system time for bundle staging: {error}"))?
        .as_nanos();
    let stem = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("glyph-bundle");
    let staging = parent.join(format!(
        ".{stem}.glyph-staging-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&staging)
        .with_context(|| format!("create bundle staging directory {}", staging.display()))?;

    let result = (|| -> Result<()> {
        let mut copied = BTreeSet::new();
        for source in &bundle.sources {
            if copied.insert(source.path.clone()) {
                copy_bundle_file(&root, &staging, &source.path)?;
            }
        }
        for asset in &bundle.assets {
            if copied.insert(asset.path.clone()) {
                copy_bundle_file(&root, &staging, &asset.path)?;
            }
        }

        let assets = bundle
            .assets
            .iter()
            .map(|asset| {
                (
                    asset.name.clone(),
                    ProjectAsset {
                        path: asset.path.clone(),
                        sha256: asset.sha256.clone(),
                    },
                )
            })
            .collect();
        let manifest = ProjectManifest {
            version: PROJECT_MANIFEST_VERSION,
            entry: Some(bundle.entry.clone()),
            assets,
        };
        let project_bytes = serde_json::to_vec_pretty(&manifest)?;
        fs::write(
            staging.join(PROJECT_MANIFEST),
            [project_bytes.as_slice(), b"\n"].concat(),
        )?;
        let bundle_bytes = serde_json::to_vec_pretty(&bundle)?;
        fs::write(
            staging.join("glyph.bundle.json"),
            [bundle_bytes.as_slice(), b"\n"].concat(),
        )?;
        validate_project(&staging.join(&bundle.entry))?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, output).with_context(|| {
        format!(
            "publish bundle staging directory {} to {}",
            staging.display(),
            output.display()
        )
    })?;
    Ok(bundle)
}
