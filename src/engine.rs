use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use serde_json::Value;

use crate::parser::JrFilter;

// for string
pub fn process_rust_value(input: &Value, filters: &[JrFilter]) -> Option<Value> {
    let mut current_results = vec![input.clone()];

    for filter in filters {
        let mut next_results: Vec<Value> = Vec::new();

        match filter {
            JrFilter::Identity => {
                next_results = current_results;
            }
            JrFilter::Select(key) => {
                for value in &current_results {
                    let obj = value.as_object()?;
                    let val = obj.get(key)?;
                    next_results.push(val.clone());
                }
            }
            JrFilter::Index(idx) => {
                for value in &current_results {
                    let arr = value.as_array()?;
                    let len = arr.len() as isize;
                    let abs_idx = if *idx < 0 {
                        len + (*idx as isize)
                    } else {
                        *idx as isize
                    };

                    if abs_idx < 0 || abs_idx >= len {
                        return None;
                    }

                    next_results.push(arr[abs_idx as usize].clone());
                }
            }
            JrFilter::Iterator => {
                for value in &current_results {
                    let arr = value.as_array()?;
                    for item in arr {
                        next_results.push(item.clone());
                    }
                }
            }
        }

        if next_results.is_empty() {
            return None;
        }

        current_results = next_results;
    }

    if current_results.len() == 1 {
        current_results.pop()
    } else {
        Some(Value::Array(current_results))
    }
}

// for object (zero copy)
pub fn process_python_object(py: Python, input: &PyAny, filters: &[JrFilter]) -> PyResult<PyObject> {
    let mut current_results: Vec<PyObject> = vec![input.to_object(py)];

    for filter in filters {
        let mut next_results: Vec<PyObject> = Vec::new();

        match filter {
            JrFilter::Identity => {
                next_results = current_results;
            }
            JrFilter::Select(key) => {
                for value in &current_results {
                    let any = value.as_ref(py);
                    if let Ok(dict) = any.downcast::<PyDict>() {
                        match dict.get_item(key)? {
                            Some(val) => next_results.push(val.to_object(py)),
                            None => return Ok(py.None()),
                        }
                    } else {
                        return Ok(py.None());
                    }
                }
            }
            JrFilter::Index(idx) => {
                for value in &current_results {
                    let any = value.as_ref(py);
                    if let Ok(list) = any.downcast::<PyList>() {
                        let len = list.len() as isize;
                        let real_index = if *idx < 0 {
                            len + (*idx as isize)
                        } else {
                            *idx as isize
                        };

                        if real_index >= 0 && real_index < len {
                            next_results.push(list.get_item(real_index as usize)?.to_object(py));
                        } else {
                            return Ok(py.None());
                        }
                    } else {
                        return Ok(py.None());
                    }
                }
            }
            JrFilter::Iterator => {
                for value in &current_results {
                    let any = value.as_ref(py);
                    if let Ok(list) = any.downcast::<PyList>() {
                        for item in list.iter() {
                            next_results.push(item.to_object(py));
                        }
                    } else {
                        return Ok(py.None());
                    }
                }
            }
        }

        if next_results.is_empty() {
            return Ok(py.None());
        }

        current_results = next_results;
    }

    if current_results.len() == 1 {
        Ok(current_results.pop().expect("current_results has one item"))
    } else {
        Ok(PyList::new(py, &current_results).to_object(py))
    }
}
