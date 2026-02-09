use pyo3::prelude::*;
use pyo3::types::{PyAny};
use serde_json::Value;
use pythonize::pythonize;

mod parser;
use parser::parse_query;

mod engine;
use engine::process_rust_value;
use engine::process_python_object;

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

#[pymodule]
fn rusty_jq(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process, m)?)?;
    Ok(())
}