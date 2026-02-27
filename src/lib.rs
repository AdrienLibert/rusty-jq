use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use simd_json::{BorrowedValue, StaticNode};
use std::borrow::Cow;

mod parser;
use parser::{parse_query, RustyFilter};

mod engine;
use engine::process_rust_value;

// converts a simd-json BorrowedValue into a native Python object
// operates on zero-copy references
// Python allocation happens at the end, so hot path stays allocation-free
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
pub struct RustyJqIter {
    items: std::vec::IntoIter<PyObject>,
}

#[pymethods]
impl RustyJqIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyObject> {
        slf.items.next()
    }
}

// compiled jq-style query, exposed to Python as RustyProgram
#[pyclass]
struct RustyProgram {
    filters: Vec<RustyFilter>,
}

#[pymethods]
impl RustyProgram {
    // execute compiled query against given JSON text
    fn input(&self, py: Python, json_text: &str) -> PyResult<RustyJqIter> {
        // converts string into a mutable buffer of bytes
        let mut bytes = json_text.as_bytes().to_vec();
        // parse the buffer in place and returns a BorrowedValue
        let json_data = simd_json::to_borrowed_value(&mut bytes)
             .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        // run filter pipeline
        let result = process_rust_value(Cow::Borrowed(&json_data), &self.filters);
        // convert each item to PyObject
        let items: PyResult<Vec<PyObject>> = result.iter()
            .map(|v| value_to_py(py, &*v))
            .collect();
            
        // Return the Iterator
        Ok(RustyJqIter { 
            items: items?.into_iter() 
        })
    }
    fn first(&self, py: Python, json_text: &str) -> PyResult<PyObject> {
        // converts string into a mutable buffer of bytes
        let mut bytes = json_text.as_bytes().to_vec();
        // parse the buffer in place and returns a BorrowedValue
        let json_data = simd_json::to_borrowed_value(&mut bytes)
             .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        // run filter pipeline
        let result = process_rust_value(Cow::Borrowed(&json_data), &self.filters);
        // grab only the very first match, avoiding overhead
        match result.first() {
            Some(val) => value_to_py(py, &*val),
            None => Ok(py.None()),
        }
    }
}

#[pyfunction]
fn compile(query: &str) -> PyResult<RustyProgram> {
    // returns IResult<&str, Vec<RustyFilter>>
    let (remaining, filters) = match parse_query(query) {
        Ok(x) => x,
        Err(e) => return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid query syntax: {}", e))),
    };

    // ensure the parser consumed the entire query string
    if !remaining.trim().is_empty() {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Extra chars"));
    }

    Ok(RustyProgram { filters })
}

// PyO3 module initialisation (entry-point)
#[pymodule]
fn rusty_jq(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compile, m)?)?;
    m.add_class::<RustyProgram>()?;
    m.add_class::<RustyJqIter>()?;
    Ok(())
}