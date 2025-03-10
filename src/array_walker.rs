use crate::types::{Display, Selection, Selections, Selector};

use rayon::prelude::*;
use serde_json::{json, Value};

/// Walks through a JSON array. Iterate over the indexes of the array, returns
/// a Result of one value or an Err early on.
pub fn array_walker(
    array_indexes: &[usize],
    inner_json: &Value,
    map_index: usize,
    selectors: &[Selector],
) -> Selection {
    let is_root = selectors.len() == 1 || map_index == 0;

    let results: Selections = array_indexes
        .par_iter()
        .map(|index| {
            // No JSON value has been found (array).
            if inner_json.get(index).is_none() {
                let error_message = match inner_json.as_array() {
                    // Trying to access an out of bound index on a node
                    // or on the root element.
                    Some(array) => {
                        if is_root {
                            [
                                "Index [",
                                index.to_string().as_str(),
                                "] is out of bound, root element has a length of ",
                                &(array.len()).to_string(),
                            ]
                            .join("")
                        } else {
                            [
                                "Index [",
                                index.to_string().as_str(),
                                "] is out of bound, ",
                                &selectors[map_index - 1].as_str(false),
                                " has a length of ",
                                &(array.len()).to_string(),
                            ]
                            .join("")
                        }
                    }
                    // Trying to access an index on a node which is not
                    // an array.
                    None => {
                        if is_root {
                            String::from("Root element is not an array")
                        } else {
                            [&selectors[map_index - 1].as_str(true), " is not an array"].join("")
                        }
                    }
                };

                return Err(error_message);
            }

            // Match found.
            Ok(inner_json.get(index).unwrap().clone())
        })
        .collect();

    match results {
        Ok(values) => Ok(if array_indexes.len() == 1 {
            // Pick the first one as there is only one index provided.
            values[0].clone()
        } else {
            // Return an array.
            json!(values)
        }),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Selector;

    #[test]
    fn valid_array_walker() {
        assert_eq!(
            Ok(json!("bar")),
            array_walker(
                &[1],
                &json!(["foo", "bar"]),
                1,
                &[Selector::Default("foo".to_string()),],
            )
        );
        assert_eq!(
            Ok(json!(["foo", "bar"])),
            array_walker(
                &[0, 1],
                &json!(["foo", "bar"]),
                1,
                &[Selector::Default("foo".to_string()),],
            )
        );
    }

    #[test]
    fn invalid_array_walker() {
        assert_eq!(
            Err(String::from(
                "Index [1] is out of bound, root element has a length of 0"
            )),
            array_walker(
                &[1],
                &json!([]),
                1,
                &[Selector::Default("foo".to_string()),],
            )
        );
        assert_eq!(
            Err(String::from(
                "Index [1] is out of bound, node \"foo\" has a length of 0"
            )),
            array_walker(
                &[1],
                &json!([]),
                1,
                &[
                    Selector::Default("foo".to_string()),
                    Selector::Index([0].to_vec()),
                ],
            )
        );
        assert_eq!(
            Err(String::from("Root element is not an array")),
            array_walker(
                &[1],
                &json!("foo"),
                1,
                &[Selector::Default("foo".to_string()),],
            )
        );
        assert_eq!(
            Err(String::from("Node \"foo\" is not an array")),
            array_walker(
                &[1],
                &json!("foo"),
                1,
                &[
                    Selector::Default("foo".to_string()),
                    Selector::Index([0].to_vec()),
                ],
            )
        );
    }
}
