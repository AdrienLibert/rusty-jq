use std::borrow::Cow;
use simd_json::BorrowedValue;
use simd_json::borrowed::Object; 
use simd_json::prelude::*;
     
use crate::parser::{RustyFilter, CompareOp, Literal, Condition, Expr};

// raw math
fn apply_op<T: PartialOrd>(a: &T, b: &T, op: &CompareOp) -> bool {
    match op {
        CompareOp::Eq => a == b,
        CompareOp::Neq => a != b,
        CompareOp::Gt => a > b,
        CompareOp::Lt => a < b,
        CompareOp::Gte => a >= b,
        CompareOp::Lte => a <= b,
    }
}

// convert a parsed Literal into a BorrowedValue so we can reuse compare_values
fn literal_to_value(lit: &Literal) -> BorrowedValue<'static> {
    match lit {
        Literal::Int(i) => BorrowedValue::Static(StaticNode::I64(*i)),
        Literal::Float(f) => BorrowedValue::Static(StaticNode::F64(*f)),
        Literal::String(s) => BorrowedValue::String(Cow::Owned(s.clone())),
        Literal::Bool(b) => BorrowedValue::Static(StaticNode::Bool(*b)),
        Literal::Null => BorrowedValue::Static(StaticNode::Null),
    }
}

fn is_truthy(val: &BorrowedValue) -> bool {
    !matches!(val, BorrowedValue::Static(StaticNode::Null) | BorrowedValue::Static(StaticNode::Bool(false)))
}

fn compare_values(left: &BorrowedValue, op: &CompareOp, right: &BorrowedValue) -> bool {
    match (left, right) {
        (BorrowedValue::Static(StaticNode::I64(a)), BorrowedValue::Static(StaticNode::I64(b))) => apply_op(a, b, op),
        (BorrowedValue::Static(StaticNode::U64(a)), BorrowedValue::Static(StaticNode::U64(b))) => apply_op(a, b, op),
        (BorrowedValue::Static(StaticNode::I64(a)), BorrowedValue::Static(StaticNode::U64(b))) => {
            match i64::try_from(*b) {
                Ok(b_i64) => apply_op(a, &b_i64, op),
                Err(_) => matches!(op, CompareOp::Neq | CompareOp::Lt | CompareOp::Lte),
            }
        }
        (BorrowedValue::Static(StaticNode::U64(a)), BorrowedValue::Static(StaticNode::I64(b))) => {
            match i64::try_from(*a) {
                Ok(a_i64) => apply_op(&a_i64, b, op),
                Err(_) => matches!(op, CompareOp::Neq | CompareOp::Gt | CompareOp::Gte),
            }
        }
        (BorrowedValue::Static(StaticNode::F64(a)), BorrowedValue::Static(StaticNode::F64(b))) => apply_op(a, b, op),
        (BorrowedValue::Static(StaticNode::I64(a)), BorrowedValue::Static(StaticNode::F64(b))) => apply_op(&(*a as f64), b, op),
        (BorrowedValue::Static(StaticNode::F64(a)), BorrowedValue::Static(StaticNode::I64(b))) => apply_op(a, &(*b as f64), op),
        (BorrowedValue::Static(StaticNode::U64(a)), BorrowedValue::Static(StaticNode::F64(b))) => apply_op(&(*a as f64), b, op),
        (BorrowedValue::Static(StaticNode::F64(a)), BorrowedValue::Static(StaticNode::U64(b))) => apply_op(a, &(*b as f64), op),
        (BorrowedValue::String(a), BorrowedValue::String(b)) => apply_op(&a.as_ref(), &b.as_ref(), op),
        (BorrowedValue::Static(StaticNode::Bool(a)), BorrowedValue::Static(StaticNode::Bool(b))) => apply_op(a, b, op),
        (BorrowedValue::Static(StaticNode::Null), BorrowedValue::Static(StaticNode::Null)) => matches!(op, CompareOp::Eq | CompareOp::Lte | CompareOp::Gte),
        _ => matches!(op, CompareOp::Neq),
    }
}

fn evaluate_condition_tree(value: &BorrowedValue, condition: &Condition) -> bool {
    match condition {
        Condition::Comparison(path, op, expr) => {
            let test_results = process_rust_value(Cow::Borrowed(value), path, None);
            let lhs = match test_results.first() {
                Some(v) => v,
                None => return false,
            };
            match expr {
                Expr::Literal(lit) => {
                    let rhs = literal_to_value(lit);
                    compare_values(lhs, op, &rhs)
                }
                Expr::Path(rhs_path) => {
                    let rhs_results = process_rust_value(Cow::Borrowed(value), rhs_path, None);
                    match rhs_results.first() {
                        Some(r) => compare_values(lhs, op, r),
                        None => false,
                    }
                }
            }
        }
        Condition::BoolPath(path) => {
            let results = process_rust_value(Cow::Borrowed(value), path, None);
            results.first().map_or(false, |v| is_truthy(v))
        }
        Condition::And(left, right) => {
            evaluate_condition_tree(value, left) && evaluate_condition_tree(value, right)
        }
        Condition::Or(left, right) => {
            evaluate_condition_tree(value, left) || evaluate_condition_tree(value, right)
        }
        Condition::Not(inner) => {
            !evaluate_condition_tree(value, inner)
        }
    }
}

// applies a chain of RustyFilter to a JSON value and returns all matching results
pub fn process_rust_value<'a>(root: Cow<'a, BorrowedValue<'a>>, filters: &[RustyFilter], limit: Option<usize>) -> Vec<Cow<'a, BorrowedValue<'a>>> {
    // seed the pipeline with the root value
    let mut current_results: Vec<Cow<'a, BorrowedValue<'a>>> = vec![root];

    for (filter_idx, filter) in filters.iter().enumerate() {
        let mut next_results = Vec::with_capacity(current_results.len());

        for value in current_results {
            match filter {
                RustyFilter::Identity => {
                    next_results.push(value);
                },
                RustyFilter::Field(key) => {
                    match value {
                        Cow::Borrowed(b_val) => {
                            if let Some(obj) = b_val.as_object() {
                                if let Some(child) = obj.get(key.as_str()) {
                                    next_results.push(Cow::Borrowed(child));
                                }
                            }
                        },
                        Cow::Owned(o_val) => {
                            // consume the owned object and move the child out
                            if let BorrowedValue::Object(mut obj) = o_val {
                                if let Some(child) = obj.remove(key.as_str()) {
                                    next_results.push(Cow::Owned(child));
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
                            // consume the owned array and move the element out
                            if let BorrowedValue::Array(mut arr) = o_val {
                                let len = arr.len() as isize;
                                let abs_idx = if *idx < 0 { len + *idx as isize } else { *idx as isize };
                                if abs_idx >= 0 && (abs_idx as usize) < (len as usize) {
                                    next_results.push(Cow::Owned(arr.swap_remove(abs_idx as usize)));
                                }
                            }
                        }
                    }
                },
                RustyFilter::Iterator => {
                    match value {
                        Cow::Borrowed(b_val) => {
                            if let Some(arr) = b_val.as_array() {
                                next_results.extend(arr.iter().map(Cow::Borrowed));
                            }
                        },
                        Cow::Owned(o_val) => {
                            // consume the owned array — move elements out, zero clones
                            if let BorrowedValue::Array(arr) = o_val {
                                next_results.extend(arr.into_iter().map(Cow::Owned));
                            }
                        }
                    }
                },
                RustyFilter::Object(pairs) => {
                    let mut product_objects: Vec<Object> = vec![Object::new()];

                    for (key, sub_query) in pairs {
                        // recursively evaluate the sub-query for this field.
                        let field_results = process_rust_value(value.clone(), sub_query, None);

                        // if any field yields no results the whole object is dropped
                        if field_results.is_empty() {
                            product_objects.clear();
                            break; 
                        }

                        let mut new_product_objects = Vec::with_capacity(product_objects.len() * field_results.len());
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
                RustyFilter::Select(condition) => {
                    if evaluate_condition_tree(&value, condition) {
                        next_results.push(value);
                    }
                }
                RustyFilter::Comma(branches) => {
                    for branch in branches {
                        next_results.extend(
                            process_rust_value(value.clone(), branch, None)
                        );
                    }
                }
            }
            if filter_idx == filters.len() - 1 {
                if let Some(lim) = limit {
                    if next_results.len() >= lim {
                        break;
                    }
                }
            }
        }
        current_results = next_results;
    }
    current_results
}