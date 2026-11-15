use crate::report::InfraDiffEntry;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn diff(declared: &Value, actual: &Value) -> Vec<InfraDiffEntry> {
    let mut declared_leaves = BTreeMap::new();
    let mut actual_leaves = BTreeMap::new();
    flatten(declared, "", &mut declared_leaves);
    flatten(actual, "", &mut actual_leaves);

    declared_leaves
        .keys()
        .chain(actual_leaves.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| {
            let declared_value = declared_leaves.get(path).cloned();
            let actual_value = actual_leaves.get(path).cloned();
            (declared_value != actual_value).then(|| InfraDiffEntry {
                path: path.clone(),
                declared: declared_value,
                actual: actual_value,
            })
        })
        .collect()
}

fn flatten(value: &Value, path: &str, leaves: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(object) if !object.is_empty() => {
            for (key, value) in object {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                flatten(value, &child_path, leaves);
            }
        }
        Value::Array(array) if !array.is_empty() => {
            for (index, value) in array.iter().enumerate() {
                flatten(value, &format!("{path}[{index}]"), leaves);
            }
        }
        Value::String(value) => {
            leaves.insert(path.to_owned(), value.clone());
        }
        Value::Number(value) => {
            leaves.insert(path.to_owned(), value.to_string());
        }
        Value::Bool(value) => {
            leaves.insert(path.to_owned(), value.to_string());
        }
        Value::Null => {
            leaves.insert(path.to_owned(), "null".to_owned());
        }
        Value::Object(_) => {
            leaves.insert(path.to_owned(), "{}".to_owned());
        }
        Value::Array(_) => {
            leaves.insert(path.to_owned(), "[]".to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::diff;
    use crate::report::InfraDiffEntry;
    use serde_json::json;

    #[test]
    fn identical_values_have_no_differences() {
        assert!(
            diff(
                &json!({"cpu": 4, "gpus": [{"model": "A"}]}),
                &json!({"cpu": 4, "gpus": [{"model": "A"}]})
            )
            .is_empty()
        );
    }

    #[test]
    fn reports_changed_leaf() {
        assert_eq!(
            diff(&json!({"image": "old"}), &json!({"image": "new"})),
            vec![InfraDiffEntry {
                path: "image".into(),
                declared: Some("old".into()),
                actual: Some("new".into()),
            }]
        );
    }

    #[test]
    fn reports_leaf_only_on_either_side() {
        assert_eq!(
            diff(&json!({"declared": true}), &json!({"actual": false})),
            vec![
                InfraDiffEntry {
                    path: "actual".into(),
                    declared: None,
                    actual: Some("false".into()),
                },
                InfraDiffEntry {
                    path: "declared".into(),
                    declared: Some("true".into()),
                    actual: None,
                },
            ]
        );
    }

    #[test]
    fn distinguishes_null_from_absent() {
        assert_eq!(
            diff(
                &json!({"value": null, "other": true}),
                &json!({"other": true})
            ),
            vec![InfraDiffEntry {
                path: "value".into(),
                declared: Some("null".into()),
                actual: None,
            }]
        );
    }

    #[test]
    fn reports_array_element_difference() {
        assert_eq!(
            diff(
                &json!({"gpus": [{"model": "A"}]}),
                &json!({"gpus": [{"model": "B"}]})
            ),
            vec![InfraDiffEntry {
                path: "gpus[0].model".into(),
                declared: Some("A".into()),
                actual: Some("B".into()),
            }]
        );
    }

    #[test]
    fn reports_nested_object_difference() {
        assert_eq!(
            diff(
                &json!({"compute": {"cpu": 4}}),
                &json!({"compute": {"cpu": 8}})
            ),
            vec![InfraDiffEntry {
                path: "compute.cpu".into(),
                declared: Some("4".into()),
                actual: Some("8".into()),
            }]
        );
    }
}
