use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use pyo3::exceptions::PyNotImplementedError;
use serde_json::Value;

use crate::parser::JrFilter;

pub fn process_rust_value(input: &Value, filters: &[JrFilter]) -> Vec<Value> {
    let mut current_results = vec![input.clone()];

    for filter in filters {
        let mut next_results: Vec<Value> = Vec::new();

        match filter {
            JrFilter::Identity => {
                next_results = current_results;
            }

            JrFilter::Select(key) => {
                for v in &current_results {
                    if let Some(obj) = v.as_object() {
                        if let Some(val) = obj.get(key) {
                            next_results.push(val.clone());
                        }
                    }
                }
            }

            JrFilter::Index(idx) => {
                for v in &current_results {
                    if let Some(arr) = v.as_array() {
                        let len = arr.len() as isize;
                        let abs_idx = if *idx < 0 { len + (*idx as isize) } else { *idx as isize };

                        if abs_idx >= 0 && abs_idx < len {
                            next_results.push(arr[abs_idx as usize].clone());
                        }
                    }
                }
            }

            JrFilter::Iterator => {
                for v in &current_results {
                    if let Some(arr) = v.as_array() {
                        for item in arr {
                            next_results.push(item.clone());
                        }
                    }
                }
            }

            JrFilter::Object(pairs) => {
                // for each current value, build zero-or-more constructed objects
                for base in &current_results {
                    let mut product_objects: Vec<serde_json::Map<String, Value>> =
                        vec![serde_json::Map::new()];

                    for (key, sub_query) in pairs {
                        let field_results = process_rust_value(base, sub_query);
                        if field_results.is_empty() {
                            product_objects.clear();
                            break;
                        }

                        let mut new_product_objects = Vec::new();
                        for partial in &product_objects {
                            for field_val in &field_results {
                                let mut obj = partial.clone();
                                obj.insert(key.clone(), field_val.clone());
                                new_product_objects.push(obj);
                            }
                        }
                        product_objects = new_product_objects;
                    }

                    for obj in product_objects {
                        next_results.push(Value::Object(obj));
                    }
                }
            }
        }

        if next_results.is_empty() {
            return vec![]; // jq-like "no output"
        }

        current_results = next_results;
    }

    current_results
}