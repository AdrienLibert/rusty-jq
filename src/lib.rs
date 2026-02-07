use pyo3::prelude::*;
use pyo3::types::PyAny;
use serde_json::Value;
use pythonize::pythonize;
use pythonize::depythonize;

mod parser;
use parser::JrFilter;
use parser::parse_identity;

#[pyfunction]

fn process(py: Python, query: &str, data: &PyAny) -> PyResult<PyObject> {
    let json_data: Value = if let Ok(json_str) = data.extract::<&str>() {
        serde_json::from_str(json_str)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?
    } else {
        depythonize(data).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("Invalid Data: {}", e))
        })?
    };
    let (remaining, filter) = match parse_identity(query) {
        Ok(x) => x,
        Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid query syntax")),
    };

    if !remaining.trim().is_empty() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
            format!("Query contained extra characters: '{}'", remaining)
        ));
    }

    let result_value = match filter {
        JrFilter::Index(idx) => {
            if let Value::Array(arr) = &json_data {
                let len = arr.len() as isize;
                let abs_index = if idx < 0 { len + idx } else { idx };
                
                if abs_index >= 0 && abs_index < len {
                    arr.get(abs_index as usize).cloned().unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            } else {
                Value::Null
            }
        },
        JrFilter::Select(field) => json_data.get(field).cloned().unwrap_or(Value::Null),
        JrFilter::Identity => json_data,
    };

    let py_result = pythonize(py, &result_value)?;
    Ok(py_result)
}

#[pymodule]
fn rusty_jq(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process, m)?)?;
    Ok(())
}