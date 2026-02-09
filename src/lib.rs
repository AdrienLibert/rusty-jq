use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use serde_json::Value;
use pythonize::pythonize;

mod parser;
use parser::JrFilter;
use parser::parse_query;

#[pyfunction]
fn process(py: Python, query: &str, input_data: &PyAny) -> PyResult<PyObject> {

    // parse filter
    let (remaining, filters) = match parse_query(query) {
        Ok(x) => x,
        Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid query syntax")),
    };

    if !remaining.trim().is_empty() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Query contained extra characters: '{}'", remaining)
        ));
    }

    // dispatch and process inputdata
    if let Ok(json_str) = input_data.extract::<&str>() {
        let json_data: Value = serde_json::from_str(json_str)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        
        let result = process_rust_value(&json_data, &filters);
        match result {
            Some(v) => Ok(pythonize(py, &v)?),
            None => Ok(py.None()),
        }

    } else {
        let result_ref = process_python_object(py, input_data, &filters)?;
        Ok(result_ref.to_object(py))
    }
}
 // for string
fn process_rust_value(mut cursor: &Value, filters: &[JrFilter]) -> Option<Value> {
    for filter in filters {
        match filter {
            JrFilter::Identity => {},
            JrFilter::Select(key) => {
                cursor = cursor.get(key)?
            },
            JrFilter::Index(idx) => {
                let arr = cursor.as_array()?;
                let len = arr.len() as isize;
                let abs_idx = if *idx < 0 { len + (*idx as isize) } else { *idx as isize };
                if abs_idx < 0 || abs_idx >= len { return None; }
                cursor = &arr[abs_idx as usize];
            }
        }
    }
    Some(cursor.clone())
}

// for object (zero copy)
fn process_python_object<'a>(py: Python<'a>, mut cursor: &'a PyAny, filters: &[JrFilter]) -> PyResult<&'a PyAny> {
    for filter in filters {
        match filter {
            JrFilter::Identity => {},
            JrFilter::Select(key) => {
                if let Ok(dict) = cursor.downcast::<PyDict>() {
                    match dict.get_item(key)? {
                        Some(val) => cursor = val,
                        None => return Ok(py.None().into_ref(py)),
                    }
                } else {
                    return Ok(py.None().into_ref(py));
                }
            },
            JrFilter::Index(idx) => {
                if let Ok(list) = cursor.downcast::<PyList>() {
                    let len = list.len();
                    let real_index = if *idx < 0 {
                        (len as isize) + (*idx as isize)
                    } else {
                        *idx as isize
                    };

                    if real_index >= 0 && (real_index as usize) < len {
                        cursor = list.get_item(real_index as usize)?;
                    } else {
                        return Ok(py.None().into_ref(py));
                    }
                } else {
                    return Ok(py.None().into_ref(py));
                }
            }
        }
    }
    Ok(cursor)
}

#[pymodule]
fn rusty_jq(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process, m)?)?;
    Ok(())
}