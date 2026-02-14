use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList};
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

    if let Ok(json_str) = input_data.extract::<&str>() {
        let json_data: Value = serde_json::from_str(json_str)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        let result = process_rust_value(&json_data, &filters);
        match result.as_slice() {
            [] => Ok(py.None()),
            [val] => Ok(pythonize(py, val)?.into_py(py)),
            _ => {
                let list = PyList::new(py, result.iter().map(|v| pythonize(py, v).unwrap()));
                Ok(list.into_py(py))
            }
        }
    }
}

#[pymodule]
fn rusty_jq(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process, m)?)?;
    Ok(())
}
