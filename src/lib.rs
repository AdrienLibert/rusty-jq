use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use simd_json::{BorrowedValue, StaticNode};
use std::borrow::Cow;

mod parser;
use parser::{parse_query, JrFilter};

mod engine;
use engine::process_rust_value;

fn value_to_py(py: Python, val: &BorrowedValue) -> PyResult<PyObject> {
    match val {
        BorrowedValue::Static(StaticNode::Null) => Ok(py.None()),
        BorrowedValue::Static(StaticNode::Bool(b)) => Ok(b.into_py(py)),
        BorrowedValue::Static(StaticNode::I64(i)) => Ok(i.into_py(py)),
        BorrowedValue::Static(StaticNode::U64(u)) => Ok(u.into_py(py)),
        BorrowedValue::Static(StaticNode::F64(f)) => Ok(f.into_py(py)),
        
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

#[pyclass]
struct JqProgram {
    filters: Vec<JrFilter>,
}

#[pymethods]
impl JqProgram {
    fn input(&self, py: Python, json_text: &str) -> PyResult<PyObject> {
        let mut bytes = json_text.as_bytes().to_vec();
        
        let json_data = simd_json::to_borrowed_value(&mut bytes)
             .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;

        let result = process_rust_value(Cow::Borrowed(&json_data), &self.filters);

        match result.as_slice() {
            [] => Ok(py.None()),
            [val] => value_to_py(py, &*val),
            _ => {
                let items: PyResult<Vec<PyObject>> = result.iter()
                    .map(|v| value_to_py(py, &*v))
                    .collect();
                Ok(PyList::new(py, items?).into_py(py))
            }
        }
    }
}

#[pyfunction]
fn compile(query: &str) -> PyResult<JqProgram> {
    let (remaining, filters) = match parse_query(query) {
        Ok(x) => x,
        Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid query syntax: {}", e))),
    };

    if !remaining.trim().is_empty() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Extra chars"));
    }

    Ok(JqProgram { filters })
}

#[pymodule]
fn rusty(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_class::<JqProgram>()?;
    Ok(())
}