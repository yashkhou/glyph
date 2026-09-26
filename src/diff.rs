use crate::model::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub node: String,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub category: String,
}

fn value_string(v: &Value) -> String {
    match v {
        Value::Number(n) => n.to_string(),
        Value::Text(s) => s.clone(),
    }
}

pub fn diff_scenes(before: &Scene, after: &Scene) -> Vec<DiffEntry> {
    let a: HashMap<_, _> = before.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let b: HashMap<_, _> = after.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut ids = BTreeSet::new();
    ids.extend(a.keys().copied());
    ids.extend(b.keys().copied());
    let mut out = Vec::new();
    for id in ids {
        match (a.get(id), b.get(id)) {
            (Some(x), Some(y)) => {
                let mut keys = BTreeSet::new();
                keys.extend(x.authored.props.keys());
                keys.extend(y.authored.props.keys());
                for key in keys {
                    let xv = x.authored.props.get(key).map(value_string);
                    let yv = y.authored.props.get(key).map(value_string);
                    if xv != yv {
                        out.push(DiffEntry {
                            node: id.into(),
                            field: key.clone(),
                            before: xv,
                            after: yv,
                            category: "authored".into(),
                        });
                    }
                }
                if x.roles != y.roles {
                    out.push(DiffEntry {
                        node: id.into(),
                        field: "roles".into(),
                        before: Some(x.roles.join(",")),
                        after: Some(y.roles.join(",")),
                        category: "semantic".into(),
                    });
                }
            }
            (Some(_), None) => out.push(DiffEntry {
                node: id.into(),
                field: "node".into(),
                before: Some("present".into()),
                after: None,
                category: "removed".into(),
            }),
            (None, Some(_)) => out.push(DiffEntry {
                node: id.into(),
                field: "node".into(),
                before: None,
                after: Some("present".into()),
                category: "added".into(),
            }),
            _ => {}
        }
    }
    out
}
