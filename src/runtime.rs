use crate::{
    model::*,
    solve_scene,
    solver::{property_value, set_property_value},
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub id: String,
    pub passed: bool,
    pub lhs: Option<f64>,
    pub target: Option<f64>,
    pub message: String,
}

pub fn evaluate_assertions(scene: &Scene) -> Vec<AssertionResult> {
    scene
        .assertions
        .iter()
        .map(|a| {
            let lhs = property_value(scene, &a.lhs);
            let rhs = match &a.rhs {
                ConstraintRhs::Number(n) => Some(*n),
                ConstraintRhs::Quantity(q) => Some(q.canonical_value()),
                ConstraintRhs::Property(r) => property_value(scene, r),
            };
            let target = rhs.map(|r| r + a.offset);
            let passed = match (lhs, target) {
                (Some(l), Some(t)) => match a.relation {
                    Relation::Eq => (l - t).abs() <= 0.0001,
                    Relation::Ge => l >= t - 0.0001,
                    Relation::Le => l <= t + 0.0001,
                },
                _ => false,
            };
            let message = if passed {
                format!("{} passed", a.id)
            } else {
                format!("{} failed: lhs {:?}, target {:?}", a.id, lhs, target)
            };
            AssertionResult {
                id: a.id.clone(),
                passed,
                lhs,
                target,
                message,
            }
        })
        .collect()
}

fn find_node_mut<'a>(scene: &'a mut Scene, r: &PropertyRef) -> Result<&'a mut Node> {
    scene
        .nodes
        .iter_mut()
        .find(|n| n.id == r.node || n.name == r.node)
        .ok_or_else(|| anyhow!("unknown node {}", r.node))
}

pub fn scene_in_state(scene: &Scene, name: &str) -> Result<Scene> {
    let state = scene
        .states
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| anyhow!("unknown state {name}"))?
        .clone();
    let mut out = scene.clone();
    for ov in &state.overrides {
        let node = find_node_mut(&mut out, &ov.target)?;
        node.authored
            .props
            .insert(ov.target.property.clone(), ov.value.clone());
    }
    out.metadata.insert("activeState".into(), name.into());
    solve_scene(&mut out);
    Ok(out)
}

fn spring(t: f64, decay: f64, frequency: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    1.0 - (-decay * t).exp() * (frequency * t).cos()
}

fn ease(name: &str, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    match name {
        "easeIn" => t * t,
        "easeOut" => 1.0 - (1.0 - t) * (1.0 - t),
        "easeInOut" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        "spring" => spring(t, 7.0, 10.0),
        "springSoft" => spring(t, 6.0, 8.0),
        "springSnappy" => spring(t, 9.0, 14.0),
        _ => t,
    }
}

fn animation_progress(animation: &AnimationSpec, time: f64) -> f64 {
    if time <= animation.delay {
        return 0.0;
    }
    if animation.duration <= 0.0 {
        return if animation.alternate && animation.repeat % 2 == 0 {
            0.0
        } else {
            1.0
        };
    }
    let elapsed = (time - animation.delay).max(0.0);
    let total = animation.duration * f64::from(animation.repeat);
    let (cycle, local) = if elapsed >= total {
        (animation.repeat.saturating_sub(1), 1.0)
    } else {
        let cycle = (elapsed / animation.duration).floor() as u32;
        let local = (elapsed - f64::from(cycle) * animation.duration) / animation.duration;
        (cycle, local)
    };
    let eased = ease(&animation.easing, local);
    if animation.alternate && cycle % 2 == 1 {
        1.0 - eased
    } else {
        eased
    }
}

pub fn sample_motion(scene: &Scene, time: f64) -> Result<Scene> {
    if !time.is_finite() || time < 0.0 {
        return Err(anyhow!("motion sample time must be finite and nonnegative"));
    }
    let mut out = scene.clone();
    solve_scene(&mut out);
    for animation in &scene.animations {
        let progress = animation_progress(animation, time);
        let value = animation.from + (animation.to - animation.from) * progress;
        set_property_value(&mut out, &animation.target, value)?;
    }
    out.metadata.insert("motionTime".into(), format!("{time}"));
    Ok(out)
}
