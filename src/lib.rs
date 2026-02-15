use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyAny};
use simd_json::BorrowedValue;
use std::borrow::Cow;

mod parser;
use parser::parse_query;

mod engine;
use engine::process_rust_value;

fn value_to_py(py: Python, val: &BorrowedValue) -> PyResult<PyObject> {
    match val {
        BorrowedValue::Static(simd_json::StaticNode::Null) => Ok(py.None()),
        BorrowedValue::Static(simd_json::StaticNode::Bool(b)) => Ok(b.into_py(py)),
        BorrowedValue::Static(simd_json::StaticNode::I64(i)) => Ok(i.into_py(py)),
        BorrowedValue::Static(simd_json::StaticNode::U64(u)) => Ok(u.into_py(py)),
        BorrowedValue::Static(simd_json::StaticNode::F64(f)) => Ok(f.into_py(py)),
        
        BorrowedValue::String(s) => Ok(s.as_ref().into_py(py)),
        
        BorrowedValue::Array(arr) => {
            let list = PyList::new(py, arr.iter().map(|item| {
                value_to_py(py, item).unwrap()
            }));
            Ok(list.into())
        },
        BorrowedValue::Object(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map.iter() {
                dict.set_item(k.as_ref(), value_to_py(py, v)?)?;
            }
            Ok(dict.into())
        }
    }
}

#[pyfunction]
fn process(py: Python, query: &str, input_data: &PyAny) -> PyResult<PyObject> {

    let (remaining, filters) = match parse_query(query) {
        Ok(x) => x,
        Err(_) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid query syntax")),
    };

    if !remaining.trim().is_empty() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Extra chars"));
    }

    if let Ok(json_str) = input_data.extract::<&str>() {
        let mut bytes = json_str.as_bytes().to_vec();
        
        let json_data = simd_json::to_borrowed_value(&mut bytes)
             .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

        let result = process_rust_value(Cow::Borrowed(&json_data), &filters);

        match result.as_slice() {
            [] => Ok(py.None()),
            [val] => value_to_py(py, &*val),
            _ => {
                let list = PyList::new(py, result.iter().map(|v| {
                    value_to_py(py, &*v).unwrap()
                }));
                Ok(list.into_py(py))
            }
        }
    } else {
        Ok(py.None())
    }
}

#[pymodule]
fn rusty_jq(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(process, m)?)?;
    Ok(())
}