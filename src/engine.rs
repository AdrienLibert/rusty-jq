use std::borrow::Cow;
use simd_json::BorrowedValue;
use simd_json::borrowed::Object; 
use simd_json::prelude::*;
     
use crate::parser::{RustyFilter, CompareOp, Literal};

// raw math
fn apply_op<T: PartialOrd + PartialEq>(a: &T, b: &T, op: &CompareOp) -> bool {
    match op {
        CompareOp::Eq => a == b,
        CompareOp::Neq => a != b,
        CompareOp::Gt => a > b,
        CompareOp::Lt => a < b,
        CompareOp::Gte => a >= b,
        CompareOp::Lte => a <= b,
    }
}

// match simd-json's types against our Literal types
fn evaluate_condition(val: &BorrowedValue, op: &CompareOp, lit: &Literal) -> bool {
    match (val, lit) {
        // Integer comparison
        (BorrowedValue::Static(StaticNode::I64(v)), Literal::Int(l)) => apply_op(v, l, op),
        (BorrowedValue::Static(StaticNode::U64(v)), Literal::Int(l)) => {
            let v_as_i64 = *v as i64; // basic comparisons
            apply_op(&v_as_i64, l, op)
        },

        // Float comparison
        (BorrowedValue::Static(StaticNode::F64(v)), Literal::Float(l)) => apply_op(v, l, op),
        
        // string comparison
        (BorrowedValue::String(v), Literal::String(l)) => apply_op(&v.as_ref(), &l.as_str(), op),
        
        // boolean comparison
        (BorrowedValue::Static(StaticNode::Bool(v)), Literal::Bool(l)) => apply_op(v, l, op),
        
        // null comparison (Only == and != make sense for null)
        (BorrowedValue::Static(StaticNode::Null), Literal::Null) => matches!(op, CompareOp::Eq),
        
        // if types completely mismatch, drop it
        _ => false,
    }
}

// applies a chain of RustyFilter to a JSON value and returns all matching results
pub fn process_rust_value<'a>(root: Cow<'a, BorrowedValue<'a>>, filters: &[RustyFilter]) -> Vec<Cow<'a, BorrowedValue<'a>>> {
    // seed the pipeline with the root value
    let mut current_results: Vec<Cow<'a, BorrowedValue<'a>>> = vec![root];

    for filter in filters {
        let mut next_results = Vec::new();

        for value in current_results {
            match filter {
                RustyFilter::Identity => {
                    next_results.push(value);
                },
                RustyFilter::Field(key) => {
                    match value {
                        // borrowed path, hand out a sub-reference without cloning
                        Cow::Borrowed(b_val) => {
                            if let Some(obj) = b_val.as_object() {
                                if let Some(child) = obj.get(key.as_str()) {
                                    next_results.push(Cow::Borrowed(child));
                                }
                            }
                        },
                        // owned path, the child must be cloned out of the owned object
                        Cow::Owned(o_val) => {
                            if let Some(obj) = o_val.as_object() {
                                if let Some(child) = obj.get(key.as_str()) {
                                    next_results.push(Cow::Owned(child.clone()));
                                }
                            }
                        }
                    }
                },
                RustyFilter::Index(idx) => {
                    match value {
                        Cow::Borrowed(b_val) => {
                            if let Some(arr) = b_val.as_array() {
                                let len = arr.len() as isize;
                                let abs_idx = if *idx < 0 { len + *idx as isize } else { *idx as isize };
                                if abs_idx >= 0 && (abs_idx as usize) < (len as usize) {
                                    next_results.push(Cow::Borrowed(&arr[abs_idx as usize]));
                                }
                            }
                        },
                        Cow::Owned(o_val) => {
                             if let Some(arr) = o_val.as_array() {
                                let len = arr.len() as isize;
                                let abs_idx = if *idx < 0 { len + *idx as isize } else { *idx as isize };
                                if abs_idx >= 0 && (abs_idx as usize) < (len as usize) {
                                    next_results.push(Cow::Owned(arr[abs_idx as usize].clone()));
                                }
                            }
                        }
                    }
                },
                RustyFilter::Iterator => {
                    match value {
                        // borrows from the parse buffer
                        Cow::Borrowed(b_val) => {
                            if let Some(arr) = b_val.as_array() {
                                next_results.extend(arr.iter().map(|v| Cow::Borrowed(v)));
                            }
                        },
                        // the parent is an owned temporary
                        Cow::Owned(o_val) => {
                            if let Some(arr) = o_val.as_array() {
                                next_results.extend(arr.iter().cloned().map(|v| Cow::Owned(v)));
                            }
                        }
                    }
                },
                RustyFilter::Object(pairs) => {
                    let mut product_objects: Vec<Object> = vec![Object::new()];

                    for (key, sub_query) in pairs {
                        // recursively evaluate the sub-query for this field.
                        let field_results = process_rust_value(value.clone(), sub_query);

                        // if any field yields no results the whole object is dropped
                        if field_results.is_empty() {
                            product_objects.clear();
                            break; 
                        }

                        let mut new_product_objects = Vec::new();
                        for partial_obj in &product_objects {
                            for field_val in &field_results {
                                let mut new_obj: Object = partial_obj.clone();
                                new_obj.insert(
                                    Cow::Owned(key.clone()), 
                                    field_val.clone().into_owned()
                                );
                                new_product_objects.push(new_obj);
                            }
                        }
                        product_objects = new_product_objects;
                    }

                    for obj in product_objects {
                        next_results.push(Cow::Owned(BorrowedValue::Object(Box::new(obj))));
                    }
                }
                RustyFilter::Select(path_filters, op, literal) => {
                    // run the left-side path against the current value
                    let test_results = process_rust_value(value.clone(), path_filters);
                    // check if the extracted value matches our literal
                    let passes = test_results.first().map_or(false, |test_val| {
                    evaluate_condition(test_val, op, literal)
                    });
                    // if it passes, KEEP the original b_val
                    if passes {
                        next_results.push(value);
                    }
                }
            }
        }
        current_results = next_results;
    }
    current_results
}