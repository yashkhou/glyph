#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, subprocess, time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "target/release/glyph"
CASES = sorted((ROOT / "benchmarks/cases").glob("*.glyph"))
GEN = ROOT / "benchmarks/generated/core-gate"
RESULT = ROOT / "benchmarks/results/core-gate.json"
GEN.mkdir(parents=True, exist_ok=True)
RESULT.parent.mkdir(parents=True, exist_ok=True)

def run(args, timeout=120):
    t0=time.perf_counter()
    p=subprocess.run([str(BIN), *map(str,args)], cwd=ROOT, capture_output=True, text=True, timeout=timeout)
    return p, round((time.perf_counter()-t0)*1000,3)

def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def load(path): return json.loads(Path(path).read_text())
def out_json(p):
    try: return json.loads(p.stdout)
    except Exception: return None

report={"artifact_cases":[],"queries":[],"patches":[],"responsive":[],"determinism":{},"failures":{},"performance":{},"solver_stress":{},"gate":{}}
compiled={}

# 1. Breadth: compile/render all curated scenes.
for src in CASES:
    stem=src.stem; gs=GEN/f"{stem}.gsir.json"; svg=GEN/f"{stem}.svg"; png=GEN/f"{stem}.png"
    chk,tchk=run(["check",src]); comp,tcomp=run(["compile",src,"-o",gs]); row={"case":stem,"check_code":chk.returncode,"check_ms":tchk,"compile_code":comp.returncode,"compile_ms":tcomp}
    if comp.returncode==0:
        d=load(gs); compiled[stem]=gs
        row.update(nodes=len(d["nodes"]), ids_unique=len({n["id"] for n in d["nodes"]})==len(d["nodes"]), all_have_roles=all(bool(n["roles"]) for n in d["nodes"]))
        sv,t=run(["render",gs,"-o",svg]); row.update(svg_code=sv.returncode,svg_ms=t,svg_sha=sha(svg) if svg.exists() else None)
    report["artifact_cases"].append(row)

# 2. Semantic role query surface.
expected={
"01_icon":"icon","02_logo":"logo","03_button":"button","04_card":"card","05_nav":"navigation","06_diagram":"connector","07_chart":"chart","08_badge":"badge","09_mascot":"mascot","10_pattern":"pattern","11_poster":"poster","12_responsive":"responsive","13_typography":"typography","14_landscape":"landscape","15_ui_screen":"ui"}
for stem,role in expected.items():
    p,t=run(["query",compiled[stem],"--role",role]); data=out_json(p)
    report["queries"].append({"case":stem,"role":role,"code":p.returncode,"ms":t,"hits":len(data) if isinstance(data,list) else 0,"ids":[x.get("id") for x in data] if isinstance(data,list) else []})

# 3. 30 independent surgical edits across different edit categories.
def apply_case(stem, idx, ops, label):
    base=compiled[stem]; dst=GEN/f"patch-{idx:02d}-{stem}.json"; p,t=run(["apply",base,"1",json.dumps(ops,separators=(",",":")),"-o",dst]); data=out_json(p) or {}
    report["patches"].append({"case":stem,"kind":label,"code":p.returncode,"ms":t,**data})

idx=0
# Numeric geometry/property edit for all 15.
for stem,gs in compiled.items():
    d=load(gs); chosen=None
    for n in d["nodes"]:
        for k,v in n["authored"]["props"].items():
            if isinstance(v,(int,float)) and not isinstance(v,bool): chosen=(n["id"],k,v+1); break
        if chosen: break
    if chosen:
        idx+=1; apply_case(stem,idx,[{"op":"set","node":chosen[0],"path":f"props.{chosen[1]}","value":chosen[2]}],"numeric")
# Fill/style edits for five scenes.
for stem in ["01_icon","04_card","08_badge","11_poster","14_landscape"]:
    d=load(compiled[stem]); n=d["nodes"][0]; idx+=1
    apply_case(stem,idx,[{"op":"set","node":n["id"],"path":"props.fill","value":"#456789"}],"style")
# Add semantic roles for five scenes.
for stem in ["02_logo","05_nav","07_chart","09_mascot","15_ui_screen"]:
    d=load(compiled[stem]); n=d["nodes"][0]; idx+=1
    apply_case(stem,idx,[{"op":"add_role","node":n["id"],"role":"benchmark-added"}],"add_role")
# Remove existing semantic roles for five scenes.
for stem in ["03_button","06_diagram","10_pattern","12_responsive","13_typography"]:
    d=load(compiled[stem]); n=d["nodes"][0]; idx+=1
    apply_case(stem,idx,[{"op":"remove_role","node":n["id"],"role":n["roles"][0]}],"remove_role")

# 4. Responsive relational intent on six constrained scenes.
for stem in ["01_icon","03_button","05_nav","08_badge","12_responsive","14_landscape"]:
    base=compiled[stem]; before=load(base); new_w=before["canvas"]["width"]*1.37; dst=GEN/f"wide-{stem}.json"
    ops=[{"op":"set_canvas","width":new_w,"height":None,"background":None}]
    p,t=run(["apply",base,"1",json.dumps(ops,separators=(",",":")),"-o",dst]); tr=out_json(p) or {}; moved=[]
    if dst.exists():
        after=load(dst); bmap={n["id"]:n for n in before["nodes"]}
        moved=[n["id"] for n in after["nodes"] if n["derived"]["props"] != bmap[n["id"]]["derived"]["props"]]
    report["responsive"].append({"case":stem,"code":p.returncode,"ms":t,"moved":moved,**tr})

# 5. Byte determinism: 20 GSIR+SVG loops for three scenes.
for stem in ["01_icon","12_responsive","14_landscape"]:
    g,s=[],[]
    for i in range(20):
        gs=GEN/f"det-{stem}-{i}.json"; svg=GEN/f"det-{stem}-{i}.svg"
        a,_=run(["compile",ROOT/f"benchmarks/cases/{stem}.glyph","-o",gs]); b,_=run(["render",gs,"-o",svg]) if a.returncode==0 else (a,0)
        if a.returncode or b.returncode: break
        g.append(sha(gs)); s.append(sha(svg))
    report["determinism"][stem]={"runs":len(g),"gsir_unique":len(set(g)),"svg_unique":len(set(s))}

# 6. Failure safety and constraint diagnostics.
fail=GEN/'failures'; fail.mkdir(exist_ok=True)
fixtures={
"malformed": 'scene "Bad" { canvas 100 100; rect a { x 1 y 2; } }',
"conflict": 'scene "Conflict" { canvas 300 200; rect a @id("a") { x 10; y 10; width 50; height 50; fill "#000"; } constrain a.center_x == 100 @required; constrain a.center_x == 200 @required; }',
"missing_ref": 'scene "Missing" { canvas 300 200; rect a @id("a") { x 10; y 10; width 50; height 50; fill "#000"; } constrain a.center_x == ghost.center_x @required; }'}
for name,txt in fixtures.items():
    fp=fail/f"{name}.glyph"; fp.write_text(txt); before=sha(fp); p,t=run(["check",fp]); report["failures"][name]={"code":p.returncode,"ms":t,"source_unchanged":sha(fp)==before,"stderr":p.stderr.strip()[:500],"stdout":p.stdout.strip()[:1000]}
# stale revision + unknown target must not create outputs.
base=compiled['01_icon']
for name,rev,ops in [
    ("stale_revision","99",[{"op":"set","node":"icon_bg","path":"props.rx","value":90}]),
    ("unknown_node","1",[{"op":"set","node":"no_such_node","path":"props.x","value":1}])]:
    dst=fail/f"{name}.json"; dst.unlink(missing_ok=True); p,t=run(["apply",base,rev,json.dumps(ops,separators=(",",":")),"-o",dst]); report["failures"][name]={"code":p.returncode,"ms":t,"output_created":dst.exists(),"stderr":p.stderr.strip()[:500]}

# 7. Deep solver dependency stress: 51 nodes / 51 required constraints.
chain=fail/'chain50.glyph'; lines=['scene "Chain50" {',' canvas 1200 300;']
for i in range(51): lines.append(f' rect n{i} @id("n{i}") {{ x {i}; y 50; width 20; height 20; fill "#444"; }}')
for i in range(50): lines.append(f' constrain n{i}.center_x == n{i+1}.center_x @required;')
lines += [' constrain n50.center_x == canvas.center_x @required;','}']; chain.write_text('\n'.join(lines)+'\n')
p,t=run(["check",chain]); gs=fail/'chain50.json'; c,tc=run(["compile",chain,"-o",gs]); centers=[]
if c.returncode==0:
    d=load(gs); centers=[n['derived']['props'].get('x',n['authored']['props'].get('x',0))+n['derived']['props'].get('width',n['authored']['props'].get('width',0))/2 for n in d['nodes']]
report['solver_stress']['chain50']={"check_code":p.returncode,"check_ms":t,"compile_code":c.returncode,"compile_ms":tc,"all_centered":bool(centers) and all(abs(x-600)<1e-9 for x in centers)}

# 8. Reserved: renderer-neutral target checks are covered by SVG/codegen tests.

# 9. Performance, three measurements each.
perf=GEN/'perf'; perf.mkdir(exist_ok=True)
for count in [100,1000,5000]:
    src=perf/f"nodes-{count}.glyph"
    lines=[f'scene "Perf {count}" {{',' canvas 2000 2000 background "#fff";']
    for i in range(count): lines.append(f' rect n{i} @id("n{i}") {{ role "perf"; x {(i%100)*18}; y {(i//100)*18}; width 12; height 12; fill "#557766"; }}')
    lines.append('}'); src.write_text('\n'.join(lines)+'\n')
    checks=[]
    for _ in range(3): p,t=run(["check",src],timeout=180); checks.append({"code":p.returncode,"ms":t})
    dst=perf/f"nodes-{count}.json"; p,t=run(["compile",src,"-o",dst],timeout=180)
    report['performance'][str(count)]={"checks":checks,"compile_code":p.returncode,"compile_ms":t}

# Evaluate only the predeclared criteria.
a=report['artifact_cases']; patches=report['patches']; resp=report['responsive']; det=report['determinism']; failures=report['failures']; perf=report['performance']
checks={
"artifact_breadth": len(a)>=12 and sum(x.get('svg_code')==0 for x in a)>=12,
"semantic_inspectability": all(x.get('ids_unique') and x.get('all_have_roles') for x in a) and all(q['code']==0 and q['hits']>0 for q in report['queries']),
"edit_locality": len(patches)>=20 and all(p['code']==0 and p.get('unrelated_authored_nodes_changed')==0 for p in patches),
"constraint_integrity": failures['conflict']['code']!=0 and failures['missing_ref']['code']!=0 and report['solver_stress']['chain50']['all_centered'],
"responsive_intent": len(resp)>=6 and all(r['code']==0 and r.get('required_constraints_violated')==0 and len(r['moved'])>0 for r in resp),
"determinism": all(det[s]['runs']==20 and det[s]['gsir_unique']==1 and det[s]['svg_unique']==1 for s in ['01_icon','12_responsive','14_landscape']),
"failure_safety": failures['malformed']['code']!=0 and failures['stale_revision']['code']!=0 and not failures['stale_revision']['output_created'] and failures['unknown_node']['code']!=0 and not failures['unknown_node']['output_created'] and all(failures[n].get('source_unchanged',True) for n in failures),
"performance_no_crash": all(v['compile_code']==0 and all(x['code']==0 for x in v['checks']) for v in perf.values()),
}
report['gate']['automated_checks']=checks
report['gate']['automated_pass']=all(checks.values())
report['gate']['visual_inspection_required']=True
RESULT.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({"automated_pass":report['gate']['automated_pass'],"checks":checks,"artifact_count":len(a),"patch_count":len(patches),"performance":report['performance']},indent=2))
raise SystemExit(0 if report['gate']['automated_pass'] else 2)
