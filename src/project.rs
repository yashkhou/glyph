use crate::{
    model::{ComponentDefinition, ImportSpec, Paint, Scene, StateOverride, Value},
    parser::{parse_import_specs, parse_scene_with_environment},
};
use anyhow::{Context, Result, anyhow};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const PROJECT_MANIFEST: &str = "glyph.project.json";
const PROJECT_MANIFEST_VERSION: u32 = 1;

fn validate_manifest(path: &Path) -> Result<()> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("read project manifest {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&source)
        .with_context(|| format!("parse project manifest {}", path.display()))?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            anyhow!(
                "project manifest {} is missing integer version",
                path.display()
            )
        })?;
    if version != u64::from(PROJECT_MANIFEST_VERSION) {
        return Err(anyhow!(
            "unsupported Glyph project manifest version {version}; current version is {PROJECT_MANIFEST_VERSION}"
        ));
    }
    Ok(())
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

fn load_recursive(path: &Path, root: &Path, stack: &mut Vec<PathBuf>) -> Result<Scene> {
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
    let source = fs::read_to_string(&path)
        .with_context(|| format!("read Glyph source {}", path.display()))?;
    let imports = parse_import_specs(&source)?;
    let mut seeded_styles = BTreeMap::new();
    let mut seeded_paints = BTreeMap::new();
    let mut seeded_components = Vec::new();
    for import in &imports {
        let target = resolve_import(&path, import, root)?;
        let imported_scene = load_recursive(&target, root, stack)?;
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
    load_recursive(&entry, &root, &mut Vec::new())
}
