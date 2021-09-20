use crate::array_walker::array_walker;
use crate::range_selector::range_selector;
use crate::types::{Display, InnerObject, Selection, Selections, Selector, Selectors};

use rayon::prelude::*;
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

fn apply_selector(
    inner_json: &Value,
    map_index: usize,
    raw_selector: &str,
    selectors: &Selectors,
) -> Selection {
    // No JSON value has been found.
    if inner_json.get(raw_selector).is_none() {
        if map_index == 0 {
            return Err([
                r#"Node ""#,
                raw_selector,
                r#"" not found on the parent element"#,
            ]
            .join(""));
        } else {
            return Err([
                r#"Node ""#,
                raw_selector,
                r#"" not found on parent "#,
                &selectors[map_index - 1].as_str(false),
            ]
            .join(""));
        }
    }

    // Default case.
    Ok(inner_json[raw_selector].clone())
}

fn object_to_vec(inner_json: &Value) -> Vec<(String, Value)> {
    // Make a mutable copy of the inner JSON.
    let mut inner_json_mut = inner_json.clone();

    inner_json_mut
        .as_object_mut()
        .unwrap()
        .to_owned()
        .into_iter()
        .collect::<Vec<(String, Value)>>()
}

/// Returns a selection based on selectors and a JSON content as a Result of
/// values or an Err early on, stopping the iteration as soon as the latter is
/// encountered.
pub fn get_selection(selectors: &Selectors, json: &Value) -> Selections {
    // Use an Arc to share the JSON data among threads.
    let data = Arc::new(Mutex::new(json.clone()));

    selectors
        .iter()
        .enumerate()
        .map(|(map_index, current_selector)| -> Selection {
            match current_selector {
                // Object selector.
                Selector::Object(properties) => {
                    properties
                        .par_iter()
                        .fold(
                            || Ok(json!({})),
                            |acc: Selection, property| {
                                match property {
                                    InnerObject::Index(indexes) => {
                                        let mut data = data.lock().unwrap();
                                        let key_and_values = object_to_vec(&data);
                                        let properties = key_and_values.len();
                                        let last_index = properties - 1;

                                        match indexes.par_iter().find_last(|&&x| x > last_index) {
                                            Some(index) => {
                                                let reference = if map_index > 0 {
                                                    selectors[map_index - 1].as_str(false)
                                                } else {
                                                    "object".to_string()
                                                };

                                                return Err([
                                                    "Index [",
                                                    index.to_string().as_str(),
                                                    "] is out of bound, ",
                                                    reference.as_str(),
                                                    " contains ",
                                                    &(properties).to_string(),
                                                    if properties == 1 {
                                                        " property"
                                                    } else {
                                                        " properties"
                                                    },
                                                ]
                                                .join(""));
                                            }
                                            None => {
                                                let map = indexes.iter().fold(
                                                    Map::with_capacity(indexes.len()),
                                                    |mut acc, index| {
                                                        acc.insert(
                                                            index.to_string(),
                                                            key_and_values[*index].1.clone(),
                                                        );

                                                        acc
                                                    },
                                                );

                                                *data = json!(map);
                                                Ok(json!(map))
                                            }
                                        }
                                    }
                                    InnerObject::Key(key) => {
                                        let data = data.lock().unwrap();

                                        match apply_selector(&data, map_index, key, selectors) {
                                            Ok(value) => match acc {
                                                Ok(mut current) => {
                                                    // Get the associated mutable Map and insert
                                                    // the property.
                                                    current
                                                        .as_object_mut()
                                                        .unwrap()
                                                        .insert(key.clone(), value);
                                                    Ok(current)
                                                }
                                                Err(error) => Err(error),
                                            },
                                            Err(error) => Err(error),
                                        }
                                    }
                                    // This selector is pretty dumb but is used as a guard
                                    // if an empty array is provided.
                                    InnerObject::Array => {
                                        let data = data.lock().unwrap();

                                        Ok(data.clone())
                                    }
                                    InnerObject::Range((start, end)) => {
                                        let data = data.lock().unwrap();
                                        let key_and_values = object_to_vec(&data);
                                        let properties = key_and_values.len();
                                        let last_index = properties - 1;
                                        let start_with_default = start.unwrap_or(0);
                                        let end_with_default = end.unwrap_or(last_index);
                                        let is_default = start_with_default < end_with_default;

                                        // Safe out of bound checks.
                                        if start_with_default > last_index
                                            || end_with_default > last_index
                                        {
                                            let reference = if map_index > 0 {
                                                selectors[map_index - 1].as_str(false)
                                            } else {
                                                "object".to_string()
                                            };

                                            return Err([
                                                "Range [",
                                                start_with_default.to_string().as_str(),
                                                ":",
                                                end_with_default.to_string().as_str(),
                                                "] is out of bound, ",
                                                reference.as_str(),
                                                " contains ",
                                                &(properties).to_string(),
                                                if properties == 1 {
                                                    " property"
                                                } else {
                                                    " properties"
                                                },
                                            ]
                                            .join(""));
                                        }

                                        let indexes = if is_default {
                                            (start_with_default..=end_with_default)
                                                .step_by(1)
                                                .collect::<Vec<usize>>()
                                        } else {
                                            (end_with_default..=start_with_default)
                                                .step_by(1)
                                                .collect::<Vec<usize>>()
                                                .into_par_iter()
                                                .rev()
                                                .collect::<Vec<usize>>()
                                        };

                                        let map = indexes.iter().fold(
                                            Map::with_capacity(indexes.len()),
                                            |mut acc, index| {
                                                acc.insert(
                                                    index.to_string(),
                                                    key_and_values[*index].1.clone(),
                                                );

                                                acc
                                            },
                                        );

                                        Ok(json!(map))
                                    }
                                }
                            },
                        )
                        .reduce(
                            || Ok(json!({})),
                            |first, second| {
                                first.and_then(|mut first| {
                                    second.map(|mut second| {
                                        first
                                            .as_object_mut()
                                            .unwrap()
                                            .extend(second.as_object_mut().unwrap().clone());

                                        first
                                    })
                                })
                            },
                        )
                }

                // Default selector.
                Selector::Default(raw_selector) => {
                    let mut data = data.lock().unwrap();

                    match apply_selector(&data, map_index, raw_selector, selectors) {
                        Ok(ref json) => {
                            *data = json.clone();
                            Ok(json.clone())
                        }
                        Err(error) => Err(error),
                    }
                }

                // Range selector.
                Selector::Range((start, end)) => {
                    let mut data = data.lock().unwrap();

                    match range_selector(
                        *end,
                        &data.clone(),
                        map_index,
                        if map_index == 0 {
                            None
                        } else {
                            Some(&selectors[map_index - 1])
                        },
                        selectors,
                        *start,
                    ) {
                        Ok(json) => {
                            *data = json.clone();
                            Ok(json)
                        }
                        Err(error) => Err(error),
                    }
                }

                // Array selector.
                Selector::Array => {
                    let mut data = data.lock().unwrap();

                    match range_selector(
                        None,
                        &data.clone(),
                        map_index,
                        if map_index == 0 {
                            None
                        } else {
                            Some(&selectors[map_index - 1])
                        },
                        selectors,
                        Some(0),
                    ) {
                        Ok(json) => {
                            *data = json.clone();
                            Ok(json)
                        }
                        Err(error) => Err(error),
                    }
                }

                // Index selector.
                Selector::Index(array_indexes) => {
                    let mut data = data.lock().unwrap();

                    match array_walker(array_indexes, &data, map_index, selectors) {
                        Ok(json) => {
                            *data = json.clone();
                            Ok(json)
                        }
                        Err(error) => Err(error),
                    }
                }
            }
        })
        .collect()
}
