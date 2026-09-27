use crate::{
    model::*,
    solver::{SolveReport, property_value},
    text_layout::apply_text_metrics,
};
use cassowary::{
    Constraint as LinearConstraint, Expression, RelationalOperator, Solver, Term, Variable,
    strength,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct VarKey {
    node: String,
    prop: String,
}

#[derive(Debug)]
struct LinearContext {
    vars: HashMap<VarKey, Variable>,
    node_lookup: HashMap<String, String>,
    canvas_width: Variable,
    canvas_height: Variable,
}

fn term(variable: Variable, coefficient: f64) -> Term {
    Term {
        variable,
        coefficient,
    }
}

fn expression(terms: Vec<Term>, constant: f64) -> Expression {
    Expression::new(terms, constant)
}

fn variable_expression(variable: Variable) -> Expression {
    expression(vec![term(variable, 1.0)], 0.0)
}

fn difference(mut lhs: Expression, rhs: Expression, offset: f64) -> Expression {
    lhs.constant -= rhs.constant + offset;
    lhs.terms.extend(
        rhs.terms
            .into_iter()
            .map(|t| term(t.variable, -t.coefficient)),
    );
    lhs
}

fn ensure_var(vars: &mut HashMap<VarKey, Variable>, node: &str, prop: &str) -> Variable {
    let key = VarKey {
        node: node.to_string(),
        prop: prop.to_string(),
    };
    *vars.entry(key).or_insert_with(Variable::new)
}

fn get_var(ctx: &LinearContext, node: &str, prop: &str) -> Option<Variable> {
    ctx.vars
        .get(&VarKey {
            node: node.to_string(),
            prop: prop.to_string(),
        })
        .copied()
}

fn is_alias(kind: &NodeKind, prop: &str) -> bool {
    match kind {
        NodeKind::Rect | NodeKind::Image => matches!(
            prop,
            "left" | "right" | "top" | "bottom" | "center_x" | "center_y"
        ),
        NodeKind::Ellipse => matches!(
            prop,
            "center_x" | "center_y" | "width" | "height" | "left" | "right" | "top" | "bottom"
        ),
        NodeKind::Text => matches!(
            prop,
            "left" | "right" | "top" | "bottom" | "center_x" | "center_y"
        ),
        NodeKind::Path | NodeKind::Group => false,
    }
}

fn prepare_scene(scene: &mut Scene) {
    for node in &mut scene.nodes {
        node.derived.props.clear();
        node.derived.relaxed_constraints.clear();
        node.derived.diagnostics.clear();
        for (k, v) in &node.authored.props {
            if let Some(n) = v.number() {
                node.derived.props.insert(k.clone(), n);
            }
        }
        apply_text_metrics(node);
    }
}

fn ensure_ref_vars(vars: &mut HashMap<VarKey, Variable>, node: &Node, prop: &str) {
    let mut ensure = |p: &str| {
        ensure_var(vars, &node.id, p);
    };
    match node.kind {
        NodeKind::Rect | NodeKind::Image => match prop {
            "x" | "left" => ensure("x"),
            "y" | "top" => ensure("y"),
            "width" => ensure("width"),
            "height" => ensure("height"),
            "right" | "center_x" => {
                ensure("x");
                ensure("width");
            }
            "bottom" | "center_y" => {
                ensure("y");
                ensure("height");
            }
            other => ensure(other),
        },
        NodeKind::Ellipse => match prop {
            "cx" | "center_x" => ensure("cx"),
            "cy" | "center_y" => ensure("cy"),
            "rx" => ensure("rx"),
            "ry" => ensure("ry"),
            "width" | "left" | "right" => {
                ensure("cx");
                ensure("rx");
            }
            "height" | "top" | "bottom" => {
                ensure("cy");
                ensure("ry");
            }
            other => ensure(other),
        },
        NodeKind::Text => match prop {
            "x" => ensure("x"),
            "y" => ensure("y"),
            "width" => ensure("width"),
            "height" => ensure("height"),
            "left" | "right" | "center_x" => {
                ensure("x");
                ensure("width");
            }
            "top" | "bottom" | "center_y" => {
                ensure("y");
                ensure("ascent");
                ensure("height");
            }
            other => ensure(other),
        },
        NodeKind::Path | NodeKind::Group => ensure(prop),
    }
}

fn build_context(scene: &Scene) -> LinearContext {
    let mut vars = HashMap::new();
    let mut node_lookup = HashMap::new();
    for node in &scene.nodes {
        node_lookup.insert(node.id.clone(), node.id.clone());
        node_lookup.insert(node.name.clone(), node.id.clone());
    }
    for c in &scene.constraints {
        for r in std::iter::once(&c.lhs).chain(match &c.rhs {
            ConstraintRhs::Property(r) => Some(r),
            ConstraintRhs::Number(_) | ConstraintRhs::Quantity(_) => None,
        }) {
            if r.node == "canvas" {
                continue;
            }
            let Some(id) = node_lookup.get(&r.node) else {
                continue;
            };
            let Some(node) = scene.nodes.iter().find(|n| &n.id == id) else {
                continue;
            };
            ensure_ref_vars(&mut vars, node, &r.property);
        }
    }
    LinearContext {
        vars,
        node_lookup,
        canvas_width: Variable::new(),
        canvas_height: Variable::new(),
    }
}

fn expr_for_ref(scene: &Scene, ctx: &LinearContext, r: &PropertyRef) -> Option<Expression> {
    if r.node == "canvas" {
        return match r.property.as_str() {
            "x" | "left" | "y" | "top" => Some(Expression::from_constant(0.0)),
            "width" | "right" => Some(variable_expression(ctx.canvas_width)),
            "height" | "bottom" => Some(variable_expression(ctx.canvas_height)),
            "center_x" => Some(expression(vec![term(ctx.canvas_width, 0.5)], 0.0)),
            "center_y" => Some(expression(vec![term(ctx.canvas_height, 0.5)], 0.0)),
            _ => None,
        };
    }
    let id = ctx.node_lookup.get(&r.node)?;
    let node = scene.nodes.iter().find(|n| &n.id == id)?;
    let v = |prop: &str| get_var(ctx, id, prop);
    let e = |prop: &str| v(prop).map(variable_expression);
    match node.kind {
        NodeKind::Rect | NodeKind::Image => match r.property.as_str() {
            "x" | "left" => e("x"),
            "y" | "top" => e("y"),
            "width" => e("width"),
            "height" => e("height"),
            "right" => Some(expression(
                vec![term(v("x")?, 1.0), term(v("width")?, 1.0)],
                0.0,
            )),
            "bottom" => Some(expression(
                vec![term(v("y")?, 1.0), term(v("height")?, 1.0)],
                0.0,
            )),
            "center_x" => Some(expression(
                vec![term(v("x")?, 1.0), term(v("width")?, 0.5)],
                0.0,
            )),
            "center_y" => Some(expression(
                vec![term(v("y")?, 1.0), term(v("height")?, 0.5)],
                0.0,
            )),
            other => e(other),
        },
        NodeKind::Ellipse => match r.property.as_str() {
            "cx" | "center_x" => e("cx"),
            "cy" | "center_y" => e("cy"),
            "rx" => e("rx"),
            "ry" => e("ry"),
            "width" => Some(expression(vec![term(v("rx")?, 2.0)], 0.0)),
            "height" => Some(expression(vec![term(v("ry")?, 2.0)], 0.0)),
            "left" => Some(expression(
                vec![term(v("cx")?, 1.0), term(v("rx")?, -1.0)],
                0.0,
            )),
            "right" => Some(expression(
                vec![term(v("cx")?, 1.0), term(v("rx")?, 1.0)],
                0.0,
            )),
            "top" => Some(expression(
                vec![term(v("cy")?, 1.0), term(v("ry")?, -1.0)],
                0.0,
            )),
            "bottom" => Some(expression(
                vec![term(v("cy")?, 1.0), term(v("ry")?, 1.0)],
                0.0,
            )),
            other => e(other),
        },
        NodeKind::Text => {
            let align = node
                .authored
                .props
                .get("align")
                .and_then(Value::text)
                .unwrap_or("start");
            match r.property.as_str() {
                "x" => e("x"),
                "y" => e("y"),
                "width" => e("width"),
                "height" => e("height"),
                "left" => match align {
                    "center" => Some(expression(
                        vec![term(v("x")?, 1.0), term(v("width")?, -0.5)],
                        0.0,
                    )),
                    "right" => Some(expression(
                        vec![term(v("x")?, 1.0), term(v("width")?, -1.0)],
                        0.0,
                    )),
                    _ => e("x"),
                },
                "right" => match align {
                    "center" => Some(expression(
                        vec![term(v("x")?, 1.0), term(v("width")?, 0.5)],
                        0.0,
                    )),
                    "right" => e("x"),
                    _ => Some(expression(
                        vec![term(v("x")?, 1.0), term(v("width")?, 1.0)],
                        0.0,
                    )),
                },
                "center_x" => match align {
                    "center" => e("x"),
                    "right" => Some(expression(
                        vec![term(v("x")?, 1.0), term(v("width")?, -0.5)],
                        0.0,
                    )),
                    _ => Some(expression(
                        vec![term(v("x")?, 1.0), term(v("width")?, 0.5)],
                        0.0,
                    )),
                },
                "top" => Some(expression(
                    vec![term(v("y")?, 1.0), term(v("ascent")?, -1.0)],
                    0.0,
                )),
                "bottom" => Some(expression(
                    vec![
                        term(v("y")?, 1.0),
                        term(v("ascent")?, -1.0),
                        term(v("height")?, 1.0),
                    ],
                    0.0,
                )),
                "center_y" => Some(expression(
                    vec![
                        term(v("y")?, 1.0),
                        term(v("ascent")?, -1.0),
                        term(v("height")?, 0.5),
                    ],
                    0.0,
                )),
                other => e(other),
            }
        }
        NodeKind::Path | NodeKind::Group => e(&r.property),
    }
}

fn relation(r: Relation) -> RelationalOperator {
    match r {
        Relation::Eq => RelationalOperator::Equal,
        Relation::Ge => RelationalOperator::GreaterOrEqual,
        Relation::Le => RelationalOperator::LessOrEqual,
    }
}

fn strength_value(s: Strength, source_order: usize, count: usize) -> f64 {
    if s == Strength::Required {
        return strength::REQUIRED;
    }
    let base = match s {
        Strength::Strong => strength::STRONG,
        Strength::Medium => strength::MEDIUM,
        Strength::Weak => strength::WEAK,
        Strength::Required => unreachable!(),
    };
    let earlier_bias = (count.saturating_sub(source_order) as f64) * 1e-9;
    base * (1.0 + earlier_bias)
}

fn add_required(solver: &mut Solver, expr: Expression, op: RelationalOperator) {
    let _ = solver.add_constraint(LinearConstraint::new(expr, op, strength::REQUIRED));
}

fn add_initial_constraints(scene: &Scene, ctx: &LinearContext, solver: &mut Solver) {
    add_required(
        solver,
        difference(
            variable_expression(ctx.canvas_width),
            Expression::from_constant(scene.canvas.width),
            0.0,
        ),
        RelationalOperator::Equal,
    );
    add_required(
        solver,
        difference(
            variable_expression(ctx.canvas_height),
            Expression::from_constant(scene.canvas.height),
            0.0,
        ),
        RelationalOperator::Equal,
    );
    let node_count = scene.nodes.len().max(1);
    for (node_index, node) in scene.nodes.iter().enumerate() {
        let authored_count = node.authored.props.len().max(1);
        for (prop_index, (prop, value)) in node.authored.props.iter().enumerate() {
            let Some(value) = value.number() else {
                continue;
            };
            if is_alias(&node.kind, prop) {
                continue;
            }
            if let Some(v) = get_var(ctx, &node.id, prop) {
                let stay = LinearConstraint::new(
                    difference(
                        variable_expression(v),
                        Expression::from_constant(value),
                        0.0,
                    ),
                    RelationalOperator::Equal,
                    strength::WEAK
                        * (0.1
                            + ((node_count - node_index) as f64 / node_count as f64) * 0.01
                            + ((authored_count - prop_index) as f64 / authored_count as f64)
                                * 0.000_001),
                );
                let _ = solver.add_constraint(stay);
            }
        }
        match node.kind {
            NodeKind::Rect | NodeKind::Image => {
                if let Some(v) = get_var(ctx, &node.id, "width") {
                    add_required(
                        solver,
                        variable_expression(v),
                        RelationalOperator::GreaterOrEqual,
                    );
                }
                if let Some(v) = get_var(ctx, &node.id, "height") {
                    add_required(
                        solver,
                        variable_expression(v),
                        RelationalOperator::GreaterOrEqual,
                    );
                }
            }
            NodeKind::Ellipse => {
                if let Some(v) = get_var(ctx, &node.id, "rx") {
                    add_required(
                        solver,
                        variable_expression(v),
                        RelationalOperator::GreaterOrEqual,
                    );
                }
                if let Some(v) = get_var(ctx, &node.id, "ry") {
                    add_required(
                        solver,
                        variable_expression(v),
                        RelationalOperator::GreaterOrEqual,
                    );
                }
            }
            NodeKind::Text => {
                for prop in ["width", "height", "ascent"] {
                    if let (Some(v), Some(value)) = (
                        get_var(ctx, &node.id, prop),
                        node.derived.props.get(prop).copied(),
                    ) {
                        add_required(
                            solver,
                            difference(
                                variable_expression(v),
                                Expression::from_constant(value),
                                0.0,
                            ),
                            RelationalOperator::Equal,
                        );
                    }
                }
            }
            NodeKind::Path | NodeKind::Group => {}
        }
    }
}

fn apply_values(scene: &mut Scene, ctx: &LinearContext, values: &HashMap<Variable, f64>) {
    for (key, variable) in &ctx.vars {
        let value = values.get(variable).copied().unwrap_or(0.0);
        if let Some(node) = scene.nodes.iter_mut().find(|n| n.id == key.node) {
            node.derived.props.insert(key.prop.clone(), value);
        }
    }
}

fn constraint_satisfied(scene: &Scene, c: &Constraint) -> Option<(bool, f64, f64)> {
    let lhs = property_value(scene, &c.lhs)?;
    let rhs = match &c.rhs {
        ConstraintRhs::Number(n) => *n,
        ConstraintRhs::Quantity(q) => q.canonical_value(),
        ConstraintRhs::Property(r) => property_value(scene, r)?,
    } + c.offset;
    let ok = match c.relation {
        Relation::Eq => (lhs - rhs).abs() <= 0.0001,
        Relation::Ge => lhs >= rhs - 0.0001,
        Relation::Le => lhs <= rhs + 0.0001,
    };
    Some((ok, lhs, rhs))
}

pub fn solve_scene_linear(scene: &mut Scene) -> SolveReport {
    prepare_scene(scene);
    let mut unresolved = scene
        .nodes
        .iter()
        .filter_map(|node| {
            node.derived
                .diagnostics
                .get("text.engine")
                .filter(|engine| engine.starts_with("coretext-error:"))
                .map(|engine| format!("text.{}:{engine}", node.id))
        })
        .collect::<Vec<_>>();
    let ctx = build_context(scene);
    let mut solver = Solver::new();
    add_initial_constraints(scene, &ctx, &mut solver);
    let mut required_conflicts = Vec::new();
    let mut added = HashSet::new();
    let count = scene.constraints.len().max(1);

    let mut constraints = scene.constraints.clone();
    constraints.sort_by_key(|c| (std::cmp::Reverse(c.strength.rank()), c.source_order));
    for c in &constraints {
        if c.lhs.node == "canvas" {
            unresolved.push(c.id.clone());
            continue;
        }
        let Some(lhs) = expr_for_ref(scene, &ctx, &c.lhs) else {
            unresolved.push(c.id.clone());
            continue;
        };
        let rhs = match &c.rhs {
            ConstraintRhs::Number(n) => Expression::from_constant(*n),
            ConstraintRhs::Quantity(q) => Expression::from_constant(q.canonical_value()),
            ConstraintRhs::Property(r) => match expr_for_ref(scene, &ctx, r) {
                Some(e) => e,
                None => {
                    unresolved.push(c.id.clone());
                    continue;
                }
            },
        };
        let linear = LinearConstraint::new(
            difference(lhs, rhs, c.offset),
            relation(c.relation),
            strength_value(c.strength, c.source_order, count),
        );
        match solver.add_constraint(linear) {
            Ok(()) => {
                added.insert(c.id.clone());
            }
            Err(err) if c.strength == Strength::Required => {
                required_conflicts.push(format!("{} is unsatisfiable: {:?}", c.id, err));
            }
            Err(_) => unresolved.push(c.id.clone()),
        }
    }

    let mut values = HashMap::new();
    for &(variable, value) in solver.fetch_changes() {
        values.insert(variable, value);
    }
    apply_values(scene, &ctx, &values);

    let mut report = SolveReport {
        applied: Vec::new(),
        relaxed: Vec::new(),
        unresolved,
        required_conflicts,
    };
    for c in &constraints {
        if report.unresolved.contains(&c.id) {
            continue;
        }
        match constraint_satisfied(scene, c) {
            Some((true, _, _)) if added.contains(&c.id) => report.applied.push(c.id.clone()),
            Some((false, lhs, rhs)) if c.strength == Strength::Required => {
                if !report.required_conflicts.iter().any(|m| m.contains(&c.id)) {
                    report.required_conflicts.push(format!(
                        "{} remains unsatisfied after solving (lhs {} vs target {})",
                        c.id, lhs, rhs
                    ));
                }
            }
            Some((false, _, _)) => report.relaxed.push(c.id.clone()),
            None => {
                if !report.unresolved.contains(&c.id) {
                    report.unresolved.push(c.id.clone());
                }
            }
            _ => {}
        }
    }
    for id in &report.relaxed {
        if let Some(c) = scene.constraints.iter().find(|c| &c.id == id) {
            if let Some(node) = scene
                .nodes
                .iter_mut()
                .find(|n| n.id == c.lhs.node || n.name == c.lhs.node)
            {
                node.derived.relaxed_constraints.push(id.clone());
            }
        }
    }
    report
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalSolveOutcome {
    pub report: SolveReport,
    pub reused_solver: bool,
    pub solve_count: u64,
    pub rebuild_count: u64,
    pub updated_stays: usize,
    pub updated_text_metrics: usize,
    pub source_constraints_reloaded: bool,
}

struct IncrementalState {
    topology: String,
    source_values: String,
    ctx: LinearContext,
    solver: Solver,
    canvas_width: f64,
    canvas_height: f64,
    canvas_width_constraint: LinearConstraint,
    canvas_height_constraint: LinearConstraint,
    stays: HashMap<VarKey, (f64, LinearConstraint)>,
    text_metrics: HashMap<VarKey, (f64, LinearConstraint)>,
    source_added: HashMap<String, LinearConstraint>,
    source_unresolved: Vec<String>,
    source_required_conflicts: Vec<String>,
    values: HashMap<Variable, f64>,
}

#[derive(Default)]
pub struct LinearSolverSession {
    state: Option<IncrementalState>,
    solve_count: u64,
    rebuild_count: u64,
}

fn session_topology_signature(scene: &Scene) -> String {
    let mut out = String::new();
    for node in &scene.nodes {
        let _ = write!(
            &mut out,
            "n:{}:{}:{:?}:[{}]|",
            node.id,
            node.name,
            node.kind,
            node.members.join(",")
        );
        for (property, value) in &node.authored.props {
            let shape = match value {
                Value::Number(_) => "number".to_string(),
                Value::Quantity(quantity) => format!("quantity:{:?}", quantity.unit),
                Value::Text(text) if property == "align" => format!("text:{text}"),
                Value::Text(_) => "text".to_string(),
            };
            let _ = write!(&mut out, "p:{property}:{shape}|");
        }
    }
    for constraint in &scene.constraints {
        let rhs = match &constraint.rhs {
            ConstraintRhs::Number(_) => "number".to_string(),
            ConstraintRhs::Quantity(quantity) => format!("quantity:{:?}", quantity.unit),
            ConstraintRhs::Property(reference) => {
                format!("property:{}.{}", reference.node, reference.property)
            }
        };
        let _ = write!(
            &mut out,
            "c:{}:{}:{}:{:?}:{:?}:{rhs}:{}|",
            constraint.id,
            constraint.lhs.node,
            constraint.lhs.property,
            constraint.relation,
            constraint.strength,
            constraint.source_order
        );
    }
    out
}

fn source_values_signature(scene: &Scene) -> String {
    let mut out = String::new();
    for constraint in &scene.constraints {
        let rhs = match &constraint.rhs {
            ConstraintRhs::Number(value) => format!("n:{}", value.to_bits()),
            ConstraintRhs::Quantity(quantity) => {
                format!("q:{}:{:?}", quantity.value.to_bits(), quantity.unit)
            }
            ConstraintRhs::Property(_) => "p".to_string(),
        };
        let _ = write!(
            &mut out,
            "{}:{}:{:?}:{rhs}|",
            constraint.id,
            constraint.offset.to_bits(),
            constraint.offset_unit
        );
    }
    out
}

fn required_equality(variable: Variable, value: f64) -> LinearConstraint {
    LinearConstraint::new(
        difference(
            variable_expression(variable),
            Expression::from_constant(value),
            0.0,
        ),
        RelationalOperator::Equal,
        strength::REQUIRED,
    )
}

fn stay_strength(
    scene: &Scene,
    node_index: usize,
    prop_index: usize,
    authored_count: usize,
) -> f64 {
    let node_count = scene.nodes.len().max(1);
    strength::WEAK
        * (0.1
            + ((node_count - node_index) as f64 / node_count as f64) * 0.01
            + ((authored_count - prop_index) as f64 / authored_count as f64) * 0.000_001)
}

fn stay_constraint(
    scene: &Scene,
    ctx: &LinearContext,
    node_index: usize,
    prop_index: usize,
    authored_count: usize,
    node: &Node,
    property: &str,
    value: f64,
) -> Option<LinearConstraint> {
    if is_alias(&node.kind, property) {
        return None;
    }
    let variable = get_var(ctx, &node.id, property)?;
    Some(LinearConstraint::new(
        difference(
            variable_expression(variable),
            Expression::from_constant(value),
            0.0,
        ),
        RelationalOperator::Equal,
        stay_strength(scene, node_index, prop_index, authored_count),
    ))
}

fn source_linear_constraint(
    scene: &Scene,
    ctx: &LinearContext,
    constraint: &Constraint,
    count: usize,
) -> Option<LinearConstraint> {
    if constraint.lhs.node == "canvas" {
        return None;
    }
    let lhs = expr_for_ref(scene, ctx, &constraint.lhs)?;
    let rhs = match &constraint.rhs {
        ConstraintRhs::Number(value) => Expression::from_constant(*value),
        ConstraintRhs::Quantity(quantity) => Expression::from_constant(quantity.canonical_value()),
        ConstraintRhs::Property(reference) => expr_for_ref(scene, ctx, reference)?,
    };
    Some(LinearConstraint::new(
        difference(lhs, rhs, constraint.offset),
        relation(constraint.relation),
        strength_value(constraint.strength, constraint.source_order, count),
    ))
}

fn add_incremental_geometry_guards(scene: &Scene, ctx: &LinearContext, solver: &mut Solver) {
    for node in &scene.nodes {
        match node.kind {
            NodeKind::Rect | NodeKind::Image => {
                for property in ["width", "height"] {
                    if let Some(variable) = get_var(ctx, &node.id, property) {
                        add_required(
                            solver,
                            variable_expression(variable),
                            RelationalOperator::GreaterOrEqual,
                        );
                    }
                }
            }
            NodeKind::Ellipse => {
                for property in ["rx", "ry"] {
                    if let Some(variable) = get_var(ctx, &node.id, property) {
                        add_required(
                            solver,
                            variable_expression(variable),
                            RelationalOperator::GreaterOrEqual,
                        );
                    }
                }
            }
            NodeKind::Text | NodeKind::Path | NodeKind::Group => {}
        }
    }
}

fn install_source_constraints(scene: &Scene, state: &mut IncrementalState) {
    state.source_added.clear();
    state.source_unresolved.clear();
    state.source_required_conflicts.clear();
    let count = scene.constraints.len().max(1);
    let mut constraints = scene.constraints.clone();
    constraints.sort_by_key(|constraint| {
        (
            std::cmp::Reverse(constraint.strength.rank()),
            constraint.source_order,
        )
    });
    for constraint in &constraints {
        let Some(linear) = source_linear_constraint(scene, &state.ctx, constraint, count) else {
            state.source_unresolved.push(constraint.id.clone());
            continue;
        };
        match state.solver.add_constraint(linear.clone()) {
            Ok(()) => {
                state.source_added.insert(constraint.id.clone(), linear);
            }
            Err(error) if constraint.strength == Strength::Required => {
                state
                    .source_required_conflicts
                    .push(format!("{} is unsatisfiable: {:?}", constraint.id, error));
            }
            Err(_) => {
                state.source_unresolved.push(constraint.id.clone());
            }
        }
    }
}

fn refresh_values(state: &mut IncrementalState, scene: &mut Scene) {
    for &(variable, value) in state.solver.fetch_changes() {
        state.values.insert(variable, value);
    }
    apply_values(scene, &state.ctx, &state.values);
}

fn incremental_report(scene: &mut Scene, state: &IncrementalState) -> SolveReport {
    let mut report = SolveReport {
        applied: Vec::new(),
        relaxed: Vec::new(),
        unresolved: scene
            .nodes
            .iter()
            .filter_map(|node| {
                node.derived
                    .diagnostics
                    .get("text.engine")
                    .filter(|engine| engine.starts_with("coretext-error:"))
                    .map(|engine| format!("text.{}:{engine}", node.id))
            })
            .collect(),
        required_conflicts: state.source_required_conflicts.clone(),
    };
    for id in &state.source_unresolved {
        if !report.unresolved.contains(id) {
            report.unresolved.push(id.clone());
        }
    }
    let mut constraints = scene.constraints.clone();
    constraints.sort_by_key(|constraint| {
        (
            std::cmp::Reverse(constraint.strength.rank()),
            constraint.source_order,
        )
    });
    for constraint in &constraints {
        if report.unresolved.contains(&constraint.id) {
            continue;
        }
        match constraint_satisfied(scene, constraint) {
            Some((true, _, _)) if state.source_added.contains_key(&constraint.id) => {
                report.applied.push(constraint.id.clone())
            }
            Some((false, lhs, rhs)) if constraint.strength == Strength::Required => {
                if !report
                    .required_conflicts
                    .iter()
                    .any(|message| message.contains(&constraint.id))
                {
                    report.required_conflicts.push(format!(
                        "{} remains unsatisfied after solving (lhs {} vs target {})",
                        constraint.id, lhs, rhs
                    ));
                }
            }
            Some((false, _, _)) => report.relaxed.push(constraint.id.clone()),
            None => report.unresolved.push(constraint.id.clone()),
            _ => {}
        }
    }
    for id in &report.relaxed {
        if let Some(constraint) = scene
            .constraints
            .iter()
            .find(|constraint| &constraint.id == id)
        {
            if let Some(node) = scene
                .nodes
                .iter_mut()
                .find(|node| node.id == constraint.lhs.node || node.name == constraint.lhs.node)
            {
                node.derived.relaxed_constraints.push(id.clone());
            }
        }
    }
    report
}

fn build_incremental_state(scene: &mut Scene) -> (IncrementalState, SolveReport) {
    prepare_scene(scene);
    let ctx = build_context(scene);
    let mut solver = Solver::new();

    let canvas_width_constraint = required_equality(ctx.canvas_width, scene.canvas.width);
    let canvas_height_constraint = required_equality(ctx.canvas_height, scene.canvas.height);
    let _ = solver.add_constraint(canvas_width_constraint.clone());
    let _ = solver.add_constraint(canvas_height_constraint.clone());

    let mut stays = HashMap::new();
    for (node_index, node) in scene.nodes.iter().enumerate() {
        let authored_count = node.authored.props.len().max(1);
        for (prop_index, (property, value)) in node.authored.props.iter().enumerate() {
            let Some(number) = value.number() else {
                continue;
            };
            let Some(constraint) = stay_constraint(
                scene,
                &ctx,
                node_index,
                prop_index,
                authored_count,
                node,
                property,
                number,
            ) else {
                continue;
            };
            let _ = solver.add_constraint(constraint.clone());
            stays.insert(
                VarKey {
                    node: node.id.clone(),
                    prop: property.clone(),
                },
                (number, constraint),
            );
        }
    }

    add_incremental_geometry_guards(scene, &ctx, &mut solver);

    let mut text_metrics = HashMap::new();
    for node in &scene.nodes {
        if node.kind != NodeKind::Text {
            continue;
        }
        for property in ["width", "height", "ascent"] {
            let Some(variable) = get_var(&ctx, &node.id, property) else {
                continue;
            };
            let Some(value) = node.derived.props.get(property).copied() else {
                continue;
            };
            let constraint = required_equality(variable, value);
            let _ = solver.add_constraint(constraint.clone());
            text_metrics.insert(
                VarKey {
                    node: node.id.clone(),
                    prop: property.to_string(),
                },
                (value, constraint),
            );
        }
    }

    let mut values = HashMap::new();
    for variable in ctx.vars.values() {
        values.insert(*variable, 0.0);
    }
    let mut state = IncrementalState {
        topology: session_topology_signature(scene),
        source_values: source_values_signature(scene),
        ctx,
        solver,
        canvas_width: scene.canvas.width,
        canvas_height: scene.canvas.height,
        canvas_width_constraint,
        canvas_height_constraint,
        stays,
        text_metrics,
        source_added: HashMap::new(),
        source_unresolved: Vec::new(),
        source_required_conflicts: Vec::new(),
        values,
    };
    install_source_constraints(scene, &mut state);
    refresh_values(&mut state, scene);
    let report = incremental_report(scene, &state);
    (state, report)
}

fn replace_required_value(
    solver: &mut Solver,
    previous: &LinearConstraint,
    variable: Variable,
    value: f64,
) -> LinearConstraint {
    let _ = solver.remove_constraint(previous);
    let next = required_equality(variable, value);
    let _ = solver.add_constraint(next.clone());
    next
}

impl LinearSolverSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn solve(&mut self, scene: &mut Scene) -> IncrementalSolveOutcome {
        self.solve_count += 1;
        let topology = session_topology_signature(scene);
        let must_rebuild = self
            .state
            .as_ref()
            .is_none_or(|state| state.topology != topology);

        if must_rebuild {
            self.rebuild_count += 1;
            let (state, report) = build_incremental_state(scene);
            self.state = Some(state);
            return IncrementalSolveOutcome {
                report,
                reused_solver: false,
                solve_count: self.solve_count,
                rebuild_count: self.rebuild_count,
                updated_stays: 0,
                updated_text_metrics: 0,
                source_constraints_reloaded: false,
            };
        }

        prepare_scene(scene);
        let state = self
            .state
            .as_mut()
            .expect("incremental state checked above");
        let mut updated_stays = 0;
        let mut updated_text_metrics = 0;

        if state.canvas_width.to_bits() != scene.canvas.width.to_bits() {
            state.canvas_width_constraint = replace_required_value(
                &mut state.solver,
                &state.canvas_width_constraint,
                state.ctx.canvas_width,
                scene.canvas.width,
            );
            state.canvas_width = scene.canvas.width;
        }
        if state.canvas_height.to_bits() != scene.canvas.height.to_bits() {
            state.canvas_height_constraint = replace_required_value(
                &mut state.solver,
                &state.canvas_height_constraint,
                state.ctx.canvas_height,
                scene.canvas.height,
            );
            state.canvas_height = scene.canvas.height;
        }

        let mut stay_updates = Vec::new();
        for (node_index, node) in scene.nodes.iter().enumerate() {
            let authored_count = node.authored.props.len().max(1);
            for (prop_index, (property, value)) in node.authored.props.iter().enumerate() {
                let Some(number) = value.number() else {
                    continue;
                };
                let key = VarKey {
                    node: node.id.clone(),
                    prop: property.clone(),
                };
                let Some((previous, _)) = state.stays.get(&key) else {
                    continue;
                };
                if previous.to_bits() == number.to_bits() {
                    continue;
                }
                if let Some(next) = stay_constraint(
                    scene,
                    &state.ctx,
                    node_index,
                    prop_index,
                    authored_count,
                    node,
                    property,
                    number,
                ) {
                    stay_updates.push((key, number, next));
                }
            }
        }
        for (key, value, next) in stay_updates {
            if let Some((_, previous)) = state.stays.remove(&key) {
                let _ = state.solver.remove_constraint(&previous);
            }
            let _ = state.solver.add_constraint(next.clone());
            state.stays.insert(key, (value, next));
            updated_stays += 1;
        }

        let mut metric_updates = Vec::new();
        for node in &scene.nodes {
            if node.kind != NodeKind::Text {
                continue;
            }
            for property in ["width", "height", "ascent"] {
                let key = VarKey {
                    node: node.id.clone(),
                    prop: property.to_string(),
                };
                let Some(value) = node.derived.props.get(property).copied() else {
                    continue;
                };
                let Some((previous, _)) = state.text_metrics.get(&key) else {
                    continue;
                };
                if previous.to_bits() != value.to_bits() {
                    metric_updates.push((key, value));
                }
            }
        }
        for (key, value) in metric_updates {
            let Some(variable) = get_var(&state.ctx, &key.node, &key.prop) else {
                continue;
            };
            let Some((_, previous)) = state.text_metrics.remove(&key) else {
                continue;
            };
            let next = replace_required_value(&mut state.solver, &previous, variable, value);
            state.text_metrics.insert(key, (value, next));
            updated_text_metrics += 1;
        }

        let next_source_values = source_values_signature(scene);
        let source_constraints_reloaded = next_source_values != state.source_values;
        if source_constraints_reloaded {
            for (_, constraint) in state.source_added.drain() {
                let _ = state.solver.remove_constraint(&constraint);
            }
            install_source_constraints(scene, state);
            state.source_values = next_source_values;
        }

        refresh_values(state, scene);
        let report = incremental_report(scene, state);
        IncrementalSolveOutcome {
            report,
            reused_solver: true,
            solve_count: self.solve_count,
            rebuild_count: self.rebuild_count,
            updated_stays,
            updated_text_metrics,
            source_constraints_reloaded,
        }
    }
}
