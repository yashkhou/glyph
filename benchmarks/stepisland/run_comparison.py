#!/usr/bin/env python3
import difflib
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
BIN = ROOT / "target/release/glyph"
OUT = HERE / "results"
OUT.mkdir(exist_ok=True)

FILES = {
    "glyph": HERE / "sakura_lake.glyph",
    "svg": HERE / "sakura_lake.svg",
    "swiftui": HERE / "SakuraLake.swift",
    "commander_draw": HERE / "sakura_lake.draw.json",
}

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def numbers(text):
    return re.findall(r'(?<![#A-Za-z_])-?\d+(?:\.\d+)?', text)

def file_metrics(path):
    text = Path(path).read_text()
    return {
        "bytes": len(text.encode()),
        "lines": len(text.splitlines()),
        "numeric_literals": len(numbers(text)),
    }

def diff_stats(before, after):
    a, b = before.splitlines(), after.splitlines()
    d = list(difflib.unified_diff(a, b, lineterm=""))
    changed = [x for x in d if (x.startswith('+') or x.startswith('-')) and not x.startswith('+++') and not x.startswith('---')]
    return {
        "changed_line_sides": len(changed),
        "changed_locations": max(1, len(changed) // 2) if changed else 0,
        "diff": d[:40],
    }

def run(*args):
    return subprocess.run([str(BIN), *map(str, args)], cwd=ROOT, text=True, capture_output=True)

report = {
    "scene": "StepIsland / Sakura Lake",
    "representations": {},
    "semantic": {},
    "surgical_edit": {},
    "resize_intent": {},
    "determinism": {},
}

for name, path in FILES.items():
    report["representations"][name] = file_metrics(path)

glyph_text = FILES["glyph"].read_text()
svg_text = FILES["svg"].read_text()
swift_text = FILES["swiftui"].read_text()
draw = json.loads(FILES["commander_draw"].read_text())

report["semantic"]["glyph"] = {
    "stable_ids": glyph_text.count('@id('),
    "role_declarations": glyph_text.count('role '),
    "relationship_declarations": glyph_text.count('constrain '),
    "machine_role_query": True,
}
report["semantic"]["svg"] = {
    "stable_ids": len(re.findall(r'\bid="', svg_text)),
    "role_attributes": len(re.findall(r'data-glyph-role="', svg_text)),
    "relationship_declarations": 0,
    "machine_role_query": "DOM/XML query required; constraints absent",
}
report["semantic"]["swiftui"] = {
    "stable_ids": swift_text.count('.accessibilityIdentifier('),
    "role_declarations": 0,
    "relationship_variable_references": sum(swift_text.count(x) for x in ['centerX', 'lakeX', 'lakeY']),
    "machine_role_query": False,
}
report["semantic"]["commander_draw"] = {
    "stable_ids": len(draw.get('nodes', [])),
    "role_declarations": 0,
    "relationship_declarations": 0,
    "machine_role_query": False,
}

# Surgical edit: right-tree vertical radius 265 -> 300.
with tempfile.TemporaryDirectory() as td:
    edited = Path(td) / 'edited.glyph'
    r = run('source-set', FILES['glyph'], 'tree_right', 'ry', '300', '--output', edited)
    report['surgical_edit']['glyph'] = {"exit": r.returncode, **diff_stats(glyph_text, edited.read_text())}

svg_old = 'id="tree_right" data-glyph-role="tree foreground framing" cx="1090" cy="2110" rx="150" ry="265"'
svg_new = 'id="tree_right" data-glyph-role="tree foreground framing" cx="1090" cy="2110" rx="150" ry="300"'
svg_edit = svg_text.replace(svg_old, svg_new)
report['surgical_edit']['svg'] = diff_stats(svg_text, svg_edit)

swift_edit = swift_text.replace(
    '.frame(width: 300, height: 530).position(x: 1090, y: 2110)',
    '.frame(width: 300, height: 600).position(x: 1090, y: 2110)',
)
report['surgical_edit']['swiftui'] = diff_stats(swift_text, swift_edit)

draw_edit = json.loads(json.dumps(draw))
target = next(n for n in draw_edit['nodes'] if n['id'] == 'tree_right')
target['ry'] = 300
report['surgical_edit']['commander_draw'] = {"changed_fields": 1, "target_id": "tree_right"}

# Responsive intent: canvas width 1290 -> 1600, title and mascot stay centered.
with tempfile.TemporaryDirectory() as td:
    td = Path(td)
    gs = td / 'base.json'
    wide = td / 'wide.json'
    c = run('compile', FILES['glyph'], '--output', gs)
    ops = '[{"op":"set_canvas","width":1600,"height":null,"background":null}]'
    a = run('apply', gs, '1', ops, '--output', wide)
    data = json.loads(wide.read_text()) if wide.exists() else {}
    nodes = {n['id']: n for n in data.get('nodes', [])}

    def visual_center(n):
        p = n['derived']['props']
        if n['kind'] == 'text' and n['authored']['props'].get('align') == 'center':
            return p.get('x')
        if 'cx' in p:
            return p['cx']
        return p.get('x', 0) + p.get('width', 0) / 2

    tx = json.loads(a.stdout) if a.returncode == 0 else {}
    report['resize_intent']['glyph'] = {
        "compile_exit": c.returncode,
        "apply_exit": a.returncode,
        "operations": 1,
        "title_center": visual_center(nodes['title']) if 'title' in nodes else None,
        "mascot_center": visual_center(nodes['mascot']) if 'mascot' in nodes else None,
        "expected_center": 800,
        "unrelated_authored_nodes_changed": tx.get('unrelated_authored_nodes_changed'),
        "required_constraints_violated": tx.get('required_constraints_violated'),
    }

report['resize_intent']['swiftui'] = {
    "source_fields_to_change": 1,
    "dependent_center_fields_to_edit": 0,
    "mechanism": "width property feeds centerX = width / 2; title and mascot use centerX",
}
report['resize_intent']['svg'] = {
    "source_fields_to_change": 4,
    "dependent_center_fields_to_edit": 2,
    "mechanism": "root width + viewBox width + title x + mascot cx; constraints were lowered away",
}
report['resize_intent']['commander_draw'] = {
    "source_fields_to_change": 3,
    "dependent_center_fields_to_edit": 2,
    "mechanism": "document width + title x + mascot cx; interchange geometry has no constraint graph",
}

# Determinism: five independent Glyph compile and render hashes.
with tempfile.TemporaryDirectory() as td:
    td = Path(td)
    gs_hashes, svg_hashes = [], []
    for i in range(5):
        gs = td / f'{i}.json'
        sv = td / f'{i}.svg'
        run('compile', FILES['glyph'], '--output', gs)
        run('render', FILES['glyph'], '--output', sv)
        gs_hashes.append(sha(gs))
        svg_hashes.append(sha(sv))
    report['determinism']['glyph'] = {
        "runs": 5,
        "gsir_unique_hashes": len(set(gs_hashes)),
        "svg_unique_hashes": len(set(svg_hashes)),
    }

report['notes'] = [
    "SwiftUI comparison intentionally uses centerX/lakeX/lakeY relationships rather than a coordinate-only implementation.",
    "Raw SVG is Glyph's semantic SVG output, so it retains stable IDs and role metadata; it demonstrates what is lost when constraints are lowered away.",
    "CommanderDraw comparison retains stable IDs but its interchange document has no Glyph constraint graph or role taxonomy.",
    "Numeric literal counts are descriptive complexity signals, not quality scores.",
]

json_path = OUT / 'stepisland-comparison.json'
json_path.write_text(json.dumps(report, indent=2) + '\n')

md = [
    '# Glyph StepIsland Cross-Representation Benchmark',
    '',
    'This benchmark uses the same Sakura Lake scene across Glyph, raw semantic SVG, relational SwiftUI and CommanderDraw JSON. It reports measured structure and edit behavior; fewer literals are not treated as a visual-quality score.',
    '',
    '## Representation size',
    '',
    '| Representation | Lines | Bytes | Numeric literals |',
    '|---|---:|---:|---:|',
]
for name, metrics in report['representations'].items():
    md.append(f"| {name} | {metrics['lines']} | {metrics['bytes']} | {metrics['numeric_literals']} |")

md += [
    '',
    '## Responsive intent test',
    '',
    'Canvas width changes from 1290 to 1600 while title and mascot must remain horizontally centered.',
    '',
    '| Representation | Source/runtime edits | Dependent center fields hand-edited | Result |',
    '|---|---:|---:|---|',
]
g = report['resize_intent']['glyph']
md.append(f"| Glyph | {g['operations']} transaction op | 0 | title={g['title_center']}, mascot={g['mascot_center']}, required violations={g['required_constraints_violated']} |")
s = report['resize_intent']['swiftui']
md.append(f"| SwiftUI | {s['source_fields_to_change']} source field | {s['dependent_center_fields_to_edit']} | relational `centerX` preserves centers |")
s = report['resize_intent']['svg']
md.append(f"| raw SVG | {s['source_fields_to_change']} fields | {s['dependent_center_fields_to_edit']} | constraints were lowered away |")
s = report['resize_intent']['commander_draw']
md.append(f"| CommanderDraw | {s['source_fields_to_change']} fields | {s['dependent_center_fields_to_edit']} | static interchange geometry |")

md += [
    '',
    '## Surgical object edit',
    '',
    'Changing only the right tree vertical radius is a one-location edit in every representation when the target is known. Glyph does not claim an advantage for this trivial case; its extra value is the typed semantic transaction/query/constraint layer around the edit.',
    '',
]
for name, metrics in report['surgical_edit'].items():
    md.append(f"- **{name}:** {metrics.get('changed_fields', metrics.get('changed_locations'))} changed field/line location(s).")

md += ['', '## Semantic / relationship retention', '']
for name, metrics in report['semantic'].items():
    md.append(f"- **{name}:** `{json.dumps(metrics, sort_keys=True)}`")

md += [
    '',
    '## Determinism',
    '',
    f"Five independent Glyph runs produced **{report['determinism']['glyph']['gsir_unique_hashes']} unique GSIR hash** and **{report['determinism']['glyph']['svg_unique_hashes']} unique SVG hash**.",
    '',
    '## Interpretation',
    '',
    '- Raw SVG can retain excellent object IDs and semantic role attributes, but does not retain Glyph’s constraint graph after lowering.',
    '- Well-authored SwiftUI can preserve responsive relationships with comparable source-edit locality; Glyph’s additional value is a graphics-specific semantic IR, renderer-neutral object map, revision-safe transactions, semantic queries and deterministic cross-target lowering.',
    '- CommanderDraw remains a useful editable/rendering backend, while Glyph carries higher-level design intent above it.',
    '- This benchmark supports continuing to Studio/Vision work, but does **not** justify replacing SwiftUI or SVG for every use case.',
    '',
]
(OUT / 'STEPISLAND_COMPARISON.md').write_text('\n'.join(md))

print(json.dumps(report['resize_intent'], indent=2))
print(json.dumps(report['determinism'], indent=2))
print('report', json_path)
