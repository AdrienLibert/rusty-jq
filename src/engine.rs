use std::borrow::Cow;
use std::cmp::Ordering;
use simd_json::BorrowedValue;
use simd_json::borrowed::Object;
use simd_json::prelude::*;

use crate::parser::{RustyFilter, CompareOp, ArithOp, Literal, Condition, Expr, Builtin0, Builtin1};

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

fn literal_to_value(lit: &Literal) -> BorrowedValue<'static> {
    match lit {
        Literal::Int(i) => BorrowedValue::Static(StaticNode::I64(*i)),
        Literal::Float(f) => BorrowedValue::Static(StaticNode::F64(*f)),
        Literal::String(s) => BorrowedValue::String(Cow::Owned(s.clone())),
        Literal::Bool(b) => BorrowedValue::Static(StaticNode::Bool(*b)),
        Literal::Null => BorrowedValue::Static(StaticNode::Null),
    }
}

fn make_array<'a>(v: Vec<BorrowedValue<'a>>) -> BorrowedValue<'a> {
    BorrowedValue::Array(Box::new(v))
}

fn apply_arith<'a>(left: &BorrowedValue, op: &ArithOp, right: &BorrowedValue) -> Option<BorrowedValue<'a>> {
    // Fast path: both sides are numbers (overwhelmingly common case)
    if let Some((a_f, b_f, a_int, b_int)) = extract_numbers(left, right) {
        return match op {
            ArithOp::Add => if a_int && b_int { Some(BorrowedValue::Static(StaticNode::I64(a_f as i64 + b_f as i64))) } else { Some(BorrowedValue::Static(StaticNode::F64(a_f + b_f))) },
            ArithOp::Sub => if a_int && b_int { Some(BorrowedValue::Static(StaticNode::I64(a_f as i64 - b_f as i64))) } else { Some(BorrowedValue::Static(StaticNode::F64(a_f - b_f))) },
            ArithOp::Mul => if a_int && b_int { Some(BorrowedValue::Static(StaticNode::I64(a_f as i64 * b_f as i64))) } else { Some(BorrowedValue::Static(StaticNode::F64(a_f * b_f))) },
            ArithOp::Div => Some(BorrowedValue::Static(StaticNode::F64(a_f / b_f))),
            ArithOp::Mod => if a_int && b_int { Some(BorrowedValue::Static(StaticNode::I64(a_f as i64 % b_f as i64))) } else { Some(BorrowedValue::Static(StaticNode::F64(a_f % b_f))) },
        };
    }
    // Slow path: type-specific operations (strings, arrays, objects, null)
    apply_arith_nonnum(left, op, right)
}

#[inline(never)]
fn apply_arith_nonnum<'a>(left: &BorrowedValue, op: &ArithOp, right: &BorrowedValue) -> Option<BorrowedValue<'a>> {
    match op {
        ArithOp::Add => {
            if let (BorrowedValue::String(a), BorrowedValue::String(b)) = (left, right) {
                let mut s = a.as_ref().to_string();
                s.push_str(b.as_ref());
                return Some(BorrowedValue::String(Cow::Owned(s)));
            }
            if let (BorrowedValue::Array(a), BorrowedValue::Array(b)) = (left, right) {
                let mut arr: Vec<BorrowedValue<'a>> = Vec::with_capacity(a.len() + b.len());
                for v in a.iter() { arr.push(clone_value(v)); }
                for v in b.iter() { arr.push(clone_value(v)); }
                return Some(make_array(arr));
            }
            if let (BorrowedValue::Object(a), BorrowedValue::Object(b)) = (left, right) {
                let mut obj = Object::with_capacity(a.len() + b.len());
                for (k, v) in a.iter() { obj.insert(Cow::Owned(k.as_ref().to_string()), clone_value(v)); }
                for (k, v) in b.iter() { obj.insert(Cow::Owned(k.as_ref().to_string()), clone_value(v)); }
                return Some(BorrowedValue::Object(Box::new(obj)));
            }
            if matches!(left, BorrowedValue::Static(StaticNode::Null)) { return Some(clone_value(right)); }
            if matches!(right, BorrowedValue::Static(StaticNode::Null)) { return Some(clone_value(left)); }
            None
        }
        ArithOp::Sub => {
            if let (BorrowedValue::Array(a), BorrowedValue::Array(b)) = (left, right) {
                let arr: Vec<BorrowedValue<'a>> = a.iter()
                    .filter(|v| !b.iter().any(|bv| values_equal(v, bv)))
                    .map(clone_value).collect();
                return Some(make_array(arr));
            }
            None
        }
        _ => None,
    }
}

fn extract_numbers(left: &BorrowedValue, right: &BorrowedValue) -> Option<(f64, f64, bool, bool)> {
    let (a_f, a_int) = to_f64(left)?;
    let (b_f, b_int) = to_f64(right)?;
    Some((a_f, b_f, a_int, b_int))
}

fn to_f64(v: &BorrowedValue) -> Option<(f64, bool)> {
    match v {
        BorrowedValue::Static(StaticNode::I64(n)) => Some((*n as f64, true)),
        BorrowedValue::Static(StaticNode::U64(n)) => Some((*n as f64, true)),
        BorrowedValue::Static(StaticNode::F64(n)) => Some((*n, false)),
        _ => None,
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
            match i64::try_from(*b) { Ok(b_i64) => apply_op(a, &b_i64, op), Err(_) => matches!(op, CompareOp::Neq | CompareOp::Lt | CompareOp::Lte) }
        }
        (BorrowedValue::Static(StaticNode::U64(a)), BorrowedValue::Static(StaticNode::I64(b))) => {
            match i64::try_from(*a) { Ok(a_i64) => apply_op(&a_i64, b, op), Err(_) => matches!(op, CompareOp::Neq | CompareOp::Gt | CompareOp::Gte) }
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
            let lhs = match test_results.first() { Some(v) => v, None => return false };
            match expr {
                Expr::Literal(lit) => { let rhs = literal_to_value(lit); compare_values(lhs, op, &rhs) }
                Expr::Path(rhs_path) => {
                    let rhs_results = process_rust_value(Cow::Borrowed(value), rhs_path, None);
                    match rhs_results.first() { Some(r) => compare_values(lhs, op, r), None => false }
                }
            }
        }
        Condition::BoolPath(path) => {
            let results = process_rust_value(Cow::Borrowed(value), path, None);
            results.first().map_or(false, |v| is_truthy(v))
        }
        Condition::And(l, r) => evaluate_condition_tree(value, l) && evaluate_condition_tree(value, r),
        Condition::Or(l, r) => evaluate_condition_tree(value, l) || evaluate_condition_tree(value, r),
        Condition::Not(inner) => !evaluate_condition_tree(value, inner),
    }
}

fn clone_value<'a>(v: &BorrowedValue) -> BorrowedValue<'a> {
    match v {
        BorrowedValue::Static(s) => BorrowedValue::Static(*s),
        BorrowedValue::String(s) => BorrowedValue::String(Cow::Owned(s.as_ref().to_string())),
        BorrowedValue::Array(arr) => make_array(arr.iter().map(clone_value).collect()),
        BorrowedValue::Object(obj) => {
            let mut new_obj = Object::with_capacity(obj.len());
            for (k, v) in obj.iter() { new_obj.insert(Cow::Owned(k.as_ref().to_string()), clone_value(v)); }
            BorrowedValue::Object(Box::new(new_obj))
        }
    }
}

fn values_equal(a: &BorrowedValue, b: &BorrowedValue) -> bool { compare_values(a, &CompareOp::Eq, b) }

fn type_order(v: &BorrowedValue) -> u8 {
    match v {
        BorrowedValue::Static(StaticNode::Null) => 0,
        BorrowedValue::Static(StaticNode::Bool(false)) => 1,
        BorrowedValue::Static(StaticNode::Bool(true)) => 2,
        BorrowedValue::Static(StaticNode::I64(_)) | BorrowedValue::Static(StaticNode::U64(_)) | BorrowedValue::Static(StaticNode::F64(_)) => 3,
        BorrowedValue::String(_) => 4,
        BorrowedValue::Array(_) => 5,
        BorrowedValue::Object(_) => 6,
    }
}

fn cmp_values(a: &BorrowedValue, b: &BorrowedValue) -> Ordering {
    let (ta, tb) = (type_order(a), type_order(b));
    if ta != tb { return ta.cmp(&tb); }
    match (a, b) {
        (BorrowedValue::Static(StaticNode::Null), _) => Ordering::Equal,
        (BorrowedValue::Static(StaticNode::Bool(x)), BorrowedValue::Static(StaticNode::Bool(y))) => x.cmp(y),
        _ if ta == 3 => {
            let af = to_f64(a).map(|x| x.0).unwrap_or(0.0);
            let bf = to_f64(b).map(|x| x.0).unwrap_or(0.0);
            af.partial_cmp(&bf).unwrap_or(Ordering::Equal)
        }
        (BorrowedValue::String(x), BorrowedValue::String(y)) => x.as_ref().cmp(y.as_ref()),
        (BorrowedValue::Array(x), BorrowedValue::Array(y)) => {
            for (a, b) in x.iter().zip(y.iter()) { let c = cmp_values(a, b); if c != Ordering::Equal { return c; } }
            x.len().cmp(&y.len())
        }
        _ => Ordering::Equal,
    }
}

fn value_to_json_string(val: &BorrowedValue) -> String {
    match val {
        BorrowedValue::Static(StaticNode::Null) => "null".to_string(),
        BorrowedValue::Static(StaticNode::Bool(b)) => b.to_string(),
        BorrowedValue::Static(StaticNode::I64(i)) => i.to_string(),
        BorrowedValue::Static(StaticNode::U64(u)) => u.to_string(),
        BorrowedValue::Static(StaticNode::F64(f)) => {
            if f.is_infinite() { if *f > 0.0 { "1.7976931348623157e+308".into() } else { "-1.7976931348623157e+308".into() } }
            else if f.is_nan() { "null".into() }
            else { format!("{}", f) }
        }
        BorrowedValue::String(s) => {
            let mut out = String::with_capacity(s.len() + 2);
            out.push('"');
            for c in s.as_ref().chars() {
                match c {
                    '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"), '\t' => out.push_str("\\t"), '\r' => out.push_str("\\r"),
                    c if (c as u32) < 0x20 => { out.push_str(&format!("\\u{:04x}", c as u32)); }
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        BorrowedValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_to_json_string).collect();
            format!("[{}]", items.join(","))
        }
        BorrowedValue::Object(obj) => {
            let items: Vec<String> = obj.iter().map(|(k, v)| format!("\"{}\":{}", k.as_ref(), value_to_json_string(v))).collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

fn value_to_string_repr(val: &BorrowedValue) -> String {
    match val {
        BorrowedValue::String(s) => s.as_ref().to_string(),
        BorrowedValue::Static(StaticNode::Null) => "null".to_string(),
        BorrowedValue::Static(StaticNode::Bool(b)) => if *b { "true" } else { "false" }.to_string(),
        BorrowedValue::Static(StaticNode::I64(i)) => i.to_string(),
        BorrowedValue::Static(StaticNode::U64(u)) => u.to_string(),
        BorrowedValue::Static(StaticNode::F64(f)) => format!("{}", f),
        _ => value_to_json_string(val),
    }
}

#[inline(never)]
fn recurse_values<'a>(val: &'a BorrowedValue<'a>, out: &mut Vec<Cow<'a, BorrowedValue<'a>>>) {
    out.push(Cow::Borrowed(val));
    match val {
        BorrowedValue::Array(arr) => { for item in arr.iter() { recurse_values(item, out); } }
        BorrowedValue::Object(obj) => { for (_, v) in obj.iter() { recurse_values(v, out); } }
        _ => {}
    }
}

#[inline(never)]
fn recurse_owned<'a>(val: BorrowedValue<'a>, out: &mut Vec<Cow<'a, BorrowedValue<'a>>>) {
    match val {
        BorrowedValue::Array(arr) => {
            for item in arr.into_iter() {
                let is_container = matches!(&item, BorrowedValue::Array(_) | BorrowedValue::Object(_));
                if is_container {
                    out.push(Cow::Owned(clone_value(&item)));
                    recurse_owned(item, out);
                } else {
                    out.push(Cow::Owned(item));
                }
            }
        }
        BorrowedValue::Object(obj) => {
            for (_, v) in obj.into_iter() {
                let is_container = matches!(&v, BorrowedValue::Array(_) | BorrowedValue::Object(_));
                if is_container {
                    out.push(Cow::Owned(clone_value(&v)));
                    recurse_owned(v, out);
                } else {
                    out.push(Cow::Owned(v));
                }
            }
        }
        _ => {}
    }
}

fn flatten_array<'a>(arr: &[BorrowedValue<'a>], depth: Option<u64>) -> Vec<BorrowedValue<'a>> {
    let mut out = Vec::new();
    for v in arr {
        if let BorrowedValue::Array(inner) = v {
            if depth.map_or(true, |d| d > 0) {
                out.extend(flatten_array(inner, depth.map(|d| d - 1)));
            } else {
                out.push(clone_value(v));
            }
        } else {
            out.push(clone_value(v));
        }
    }
    out
}

fn resolve_slice_index(idx: i64, len: i64) -> usize {
    let i = if idx < 0 { (len + idx).max(0) } else { idx.min(len) };
    i as usize
}

fn value_contains(a: &BorrowedValue, b: &BorrowedValue) -> bool {
    match (a, b) {
        (BorrowedValue::String(a), BorrowedValue::String(b)) => a.as_ref().contains(b.as_ref()),
        (BorrowedValue::Array(a), BorrowedValue::Array(b)) => b.iter().all(|bv| a.iter().any(|av| value_contains(av, bv))),
        (BorrowedValue::Object(a), BorrowedValue::Object(b)) => b.iter().all(|(bk, bv)| a.get(bk.as_ref()).map_or(false, |av| value_contains(av, bv))),
        _ => values_equal(a, b),
    }
}

fn owned_to_borrowed<'a>(val: simd_json::OwnedValue) -> BorrowedValue<'a> {
    match val {
        simd_json::OwnedValue::Static(s) => BorrowedValue::Static(s),
        simd_json::OwnedValue::String(s) => BorrowedValue::String(Cow::Owned(s)),
        simd_json::OwnedValue::Array(arr) => make_array(arr.into_iter().map(owned_to_borrowed).collect()),
        simd_json::OwnedValue::Object(obj) => {
            let mut new_obj = Object::with_capacity(obj.len());
            for (k, v) in obj.into_iter() { new_obj.insert(Cow::Owned(k.to_string()), owned_to_borrowed(v)); }
            BorrowedValue::Object(Box::new(new_obj))
        }
    }
}

// ─── main pipeline ─────────────────────────────────────────────────────────────

pub fn process_rust_value<'a>(root: Cow<'a, BorrowedValue<'a>>, filters: &[RustyFilter], limit: Option<usize>) -> Vec<Cow<'a, BorrowedValue<'a>>> {
    let mut current_results: Vec<Cow<'a, BorrowedValue<'a>>> = vec![root];

    for (filter_idx, filter) in filters.iter().enumerate() {
        let mut next_results = Vec::with_capacity(current_results.len());

        for value in current_results {
            match filter {
                RustyFilter::Identity => { next_results.push(value); }
                RustyFilter::Field(key) => {
                    match value {
                        Cow::Borrowed(b_val) => {
                            if let Some(obj) = b_val.as_object() {
                                if let Some(child) = obj.get(key.as_str()) { next_results.push(Cow::Borrowed(child)); }
                            }
                        }
                        Cow::Owned(o_val) => {
                            if let BorrowedValue::Object(mut obj) = o_val {
                                if let Some(child) = obj.remove(key.as_str()) { next_results.push(Cow::Owned(child)); }
                            }
                        }
                    }
                }
                RustyFilter::Index(idx) => {
                    match value {
                        Cow::Borrowed(b_val) => {
                            if let Some(arr) = b_val.as_array() {
                                let len = arr.len() as isize;
                                let abs_idx = if *idx < 0 { len + *idx as isize } else { *idx as isize };
                                if abs_idx >= 0 && (abs_idx as usize) < (len as usize) { next_results.push(Cow::Borrowed(&arr[abs_idx as usize])); }
                            }
                        }
                        Cow::Owned(o_val) => {
                            if let BorrowedValue::Array(mut arr) = o_val {
                                let len = arr.len() as isize;
                                let abs_idx = if *idx < 0 { len + *idx as isize } else { *idx as isize };
                                if abs_idx >= 0 && (abs_idx as usize) < (len as usize) { next_results.push(Cow::Owned(arr.swap_remove(abs_idx as usize))); }
                            }
                        }
                    }
                }
                RustyFilter::Iterator => {
                    match value {
                        Cow::Borrowed(b_val) => {
                            if let Some(arr) = b_val.as_array() { next_results.extend(arr.iter().map(Cow::Borrowed)); }
                            else if let Some(obj) = b_val.as_object() { next_results.extend(obj.values().map(Cow::Borrowed)); }
                        }
                        Cow::Owned(o_val) => {
                            match o_val {
                                BorrowedValue::Array(arr) => { next_results.extend(arr.into_iter().map(Cow::Owned)); }
                                BorrowedValue::Object(obj) => { next_results.extend(obj.into_iter().map(|(_, v)| Cow::Owned(v))); }
                                _ => {}
                            }
                        }
                    }
                }
                RustyFilter::Object(pairs) => {
                    let mut product_objects: Vec<Object> = vec![Object::new()];
                    for (key, sub_query) in pairs {
                        let field_results = process_rust_value(value.clone(), sub_query, None);
                        if field_results.is_empty() { product_objects.clear(); break; }
                        let mut new_product_objects = Vec::with_capacity(product_objects.len() * field_results.len());
                        for partial_obj in &product_objects {
                            for field_val in &field_results {
                                let mut new_obj: Object = partial_obj.clone();
                                new_obj.insert(Cow::Owned(key.clone()), field_val.clone().into_owned());
                                new_product_objects.push(new_obj);
                            }
                        }
                        product_objects = new_product_objects;
                    }
                    for obj in product_objects { next_results.push(Cow::Owned(BorrowedValue::Object(Box::new(obj)))); }
                }
                RustyFilter::Select(condition) => {
                    if evaluate_condition_tree(&value, condition) { next_results.push(value); }
                }
                RustyFilter::Comma(branches) => {
                    for branch in branches { next_results.extend(process_rust_value(value.clone(), branch, None)); }
                }
                RustyFilter::LiteralValue(lit) => { next_results.push(Cow::Owned(literal_to_value(lit))); }
                RustyFilter::Arithmetic(left, op, right) => {
                    let left_results = process_rust_value(value.clone(), left, None);
                    let right_results = process_rust_value(value, right, None);
                    if let (Some(lv), Some(rv)) = (left_results.first(), right_results.first()) {
                        if let Some(result) = apply_arith(lv, op, rv) { next_results.push(Cow::Owned(result)); }
                    }
                }
                RustyFilter::RecurseDescent => {
                    match &value {
                        Cow::Borrowed(b_val) => { recurse_values(b_val, &mut next_results); }
                        Cow::Owned(_) => {
                            let owned = value.into_owned();
                            next_results.push(Cow::Owned(clone_value(&owned)));
                            recurse_owned(owned, &mut next_results);
                        }
                    }
                }
                RustyFilter::Slice(start, end) => {
                    match &*value {
                        BorrowedValue::Array(arr) => {
                            let len = arr.len() as i64;
                            let s = resolve_slice_index(start.unwrap_or(0), len);
                            let e = resolve_slice_index(end.unwrap_or(len), len);
                            let sliced: Vec<BorrowedValue<'a>> = if s < e { arr[s..e].iter().map(clone_value).collect() } else { Vec::new() };
                            next_results.push(Cow::Owned(make_array(sliced)));
                        }
                        BorrowedValue::String(s_val) => {
                            let chars: Vec<char> = s_val.as_ref().chars().collect();
                            let len = chars.len() as i64;
                            let s = resolve_slice_index(start.unwrap_or(0), len);
                            let e = resolve_slice_index(end.unwrap_or(len), len);
                            let sliced: String = if s < e { chars[s..e].iter().collect() } else { String::new() };
                            next_results.push(Cow::Owned(BorrowedValue::String(Cow::Owned(sliced))));
                        }
                        _ => {}
                    }
                }
                RustyFilter::Builtin0(b) => { exec_builtin0(b, value, &mut next_results); }
                RustyFilter::Builtin1(b, arg) => { exec_builtin1(b, arg, value, &mut next_results); }
            }
            if filter_idx == filters.len() - 1 {
                if let Some(lim) = limit { if next_results.len() >= lim { break; } }
            }
        }
        current_results = next_results;
    }
    current_results
}

// ─── builtin0 (no-arg) ─────────────────────────────────────────────────────────

#[inline(never)]
fn exec_builtin0<'a>(b: &Builtin0, value: Cow<'a, BorrowedValue<'a>>, out: &mut Vec<Cow<'a, BorrowedValue<'a>>>) {
    match b {
        Builtin0::Length => {
            let n: i64 = match &*value {
                BorrowedValue::Array(a) => a.len() as i64,
                BorrowedValue::Object(o) => o.len() as i64,
                BorrowedValue::String(s) => s.as_ref().chars().count() as i64,
                BorrowedValue::Static(StaticNode::Null) => 0,
                BorrowedValue::Static(StaticNode::I64(i)) => i.abs(),
                BorrowedValue::Static(StaticNode::U64(u)) => *u as i64,
                BorrowedValue::Static(StaticNode::F64(f)) => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::F64(f.abs())))); return; }
                _ => return,
            };
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(n))));
        }
        Builtin0::Keys => {
            match &*value {
                BorrowedValue::Object(obj) => {
                    let mut keys: Vec<&str> = obj.keys().map(|k| k.as_ref()).collect();
                    keys.sort();
                    let arr: Vec<BorrowedValue> = keys.into_iter().map(|k| BorrowedValue::String(Cow::Owned(k.to_string()))).collect();
                    out.push(Cow::Owned(make_array(arr)));
                }
                BorrowedValue::Array(arr) => {
                    let indices: Vec<BorrowedValue> = (0..arr.len() as i64).map(|i| BorrowedValue::Static(StaticNode::I64(i))).collect();
                    out.push(Cow::Owned(make_array(indices)));
                }
                _ => {}
            }
        }
        Builtin0::KeysUnsorted => {
            if let BorrowedValue::Object(obj) = &*value {
                let arr: Vec<BorrowedValue> = obj.keys().map(|k| BorrowedValue::String(Cow::Owned(k.as_ref().to_string()))).collect();
                out.push(Cow::Owned(make_array(arr)));
            }
        }
        Builtin0::Values => {
            match &*value {
                BorrowedValue::Object(obj) => {
                    let arr: Vec<BorrowedValue> = obj.values().map(clone_value).collect();
                    out.push(Cow::Owned(make_array(arr)));
                }
                BorrowedValue::Array(_) => { out.push(value); }
                _ => {}
            }
        }
        Builtin0::Type => {
            let t = match &*value {
                BorrowedValue::Static(StaticNode::Null) => "null",
                BorrowedValue::Static(StaticNode::Bool(_)) => "boolean",
                BorrowedValue::Static(StaticNode::I64(_)) | BorrowedValue::Static(StaticNode::U64(_)) | BorrowedValue::Static(StaticNode::F64(_)) => "number",
                BorrowedValue::String(_) => "string",
                BorrowedValue::Array(_) => "array",
                BorrowedValue::Object(_) => "object",
            };
            out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(t.to_string()))));
        }
        Builtin0::Reverse => {
            match &*value {
                BorrowedValue::Array(arr) => {
                    let mut reversed: Vec<BorrowedValue> = arr.iter().map(clone_value).collect();
                    reversed.reverse();
                    out.push(Cow::Owned(make_array(reversed)));
                }
                BorrowedValue::String(s) => {
                    let rev: String = s.as_ref().chars().rev().collect();
                    out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(rev))));
                }
                _ => {}
            }
        }
        Builtin0::Sort => {
            if let BorrowedValue::Array(arr) = &*value {
                let mut sorted: Vec<BorrowedValue> = arr.iter().map(clone_value).collect();
                sorted.sort_by(cmp_values);
                out.push(Cow::Owned(make_array(sorted)));
            }
        }
        Builtin0::Flatten => {
            if let BorrowedValue::Array(arr) = &*value {
                out.push(Cow::Owned(make_array(flatten_array(arr, None))));
            }
        }
        Builtin0::Add => {
            if let BorrowedValue::Array(arr) = &*value {
                if arr.is_empty() {
                    out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null)));
                } else {
                    let mut acc = clone_value(&arr[0]);
                    for item in arr.iter().skip(1) {
                        if let Some(result) = apply_arith(&acc, &ArithOp::Add, item) { acc = result; }
                    }
                    out.push(Cow::Owned(acc));
                }
            }
        }
        Builtin0::Min => {
            if let BorrowedValue::Array(arr) = &*value {
                if let Some(m) = arr.iter().min_by(|a, b| cmp_values(a, b)) { out.push(Cow::Owned(clone_value(m))); }
            }
        }
        Builtin0::Max => {
            if let BorrowedValue::Array(arr) = &*value {
                if let Some(m) = arr.iter().max_by(|a, b| cmp_values(a, b)) { out.push(Cow::Owned(clone_value(m))); }
            }
        }
        Builtin0::Unique => {
            if let BorrowedValue::Array(arr) = &*value {
                let mut sorted: Vec<BorrowedValue> = arr.iter().map(clone_value).collect();
                sorted.sort_by(cmp_values);
                sorted.dedup_by(|a, b| values_equal(a, b));
                out.push(Cow::Owned(make_array(sorted)));
            }
        }
        Builtin0::First => {
            if let BorrowedValue::Array(arr) = &*value {
                if let Some(v) = arr.first() { out.push(Cow::Owned(clone_value(v))); }
            }
        }
        Builtin0::Last => {
            if let BorrowedValue::Array(arr) = &*value {
                if let Some(v) = arr.last() { out.push(Cow::Owned(clone_value(v))); }
            }
        }
        Builtin0::Not => {
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(!is_truthy(&value)))));
        }
        Builtin0::Empty => { /* zero outputs */ }
        Builtin0::Tostring => {
            out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(value_to_string_repr(&value)))));
        }
        Builtin0::Tonumber => {
            match &*value {
                BorrowedValue::Static(StaticNode::I64(_)) | BorrowedValue::Static(StaticNode::U64(_)) | BorrowedValue::Static(StaticNode::F64(_)) => { out.push(value); }
                BorrowedValue::String(s) => {
                    let s = s.as_ref().trim();
                    if let Ok(i) = s.parse::<i64>() { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(i)))); }
                    else if let Ok(f) = s.parse::<f64>() { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::F64(f)))); }
                }
                _ => {}
            }
        }
        Builtin0::ToEntries => {
            if let BorrowedValue::Object(obj) = &*value {
                let arr: Vec<BorrowedValue> = obj.iter().map(|(k, v)| {
                    let mut entry = Object::with_capacity(2);
                    entry.insert(Cow::Owned("key".to_string()), BorrowedValue::String(Cow::Owned(k.as_ref().to_string())));
                    entry.insert(Cow::Owned("value".to_string()), clone_value(v));
                    BorrowedValue::Object(Box::new(entry))
                }).collect();
                out.push(Cow::Owned(make_array(arr)));
            }
        }
        Builtin0::FromEntries => {
            if let BorrowedValue::Array(arr) = &*value {
                let mut obj = Object::with_capacity(arr.len());
                for item in arr.iter() {
                    if let BorrowedValue::Object(entry) = item {
                        let key = entry.get("key").or_else(|| entry.get("name")).and_then(|v| match v {
                            BorrowedValue::String(s) => Some(s.as_ref().to_string()),
                            BorrowedValue::Static(StaticNode::I64(i)) => Some(i.to_string()),
                            BorrowedValue::Static(StaticNode::U64(u)) => Some(u.to_string()),
                            _ => None,
                        });
                        if let Some(k) = key {
                            let val = entry.get("value").map(clone_value).unwrap_or(BorrowedValue::Static(StaticNode::Null));
                            obj.insert(Cow::Owned(k), val);
                        }
                    }
                }
                out.push(Cow::Owned(BorrowedValue::Object(Box::new(obj))));
            }
        }
        Builtin0::AsciiDowncase => {
            if let BorrowedValue::String(s) = &*value {
                out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(s.as_ref().to_ascii_lowercase()))));
            }
        }
        Builtin0::AsciiUpcase => {
            if let BorrowedValue::String(s) = &*value {
                out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(s.as_ref().to_ascii_uppercase()))));
            }
        }
        Builtin0::Tojson => {
            out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(value_to_json_string(&value)))));
        }
        Builtin0::Fromjson => {
            if let BorrowedValue::String(s) = &*value {
                let mut bytes = s.as_ref().as_bytes().to_vec();
                if let Ok(parsed) = simd_json::to_owned_value(&mut bytes) {
                    out.push(Cow::Owned(owned_to_borrowed(parsed)));
                }
            }
        }
        Builtin0::Explode => {
            if let BorrowedValue::String(s) = &*value {
                let codepoints: Vec<BorrowedValue> = s.as_ref().chars().map(|c| BorrowedValue::Static(StaticNode::I64(c as i64))).collect();
                out.push(Cow::Owned(make_array(codepoints)));
            }
        }
        Builtin0::Implode => {
            if let BorrowedValue::Array(arr) = &*value {
                let s: String = arr.iter().filter_map(|v| match v {
                    BorrowedValue::Static(StaticNode::I64(i)) => char::from_u32(*i as u32),
                    BorrowedValue::Static(StaticNode::U64(u)) => char::from_u32(*u as u32),
                    _ => None,
                }).collect();
                out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(s))));
            }
        }
        Builtin0::Floor => {
            match &*value {
                BorrowedValue::Static(StaticNode::F64(f)) => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(f.floor() as i64)))); }
                BorrowedValue::Static(StaticNode::I64(_)) | BorrowedValue::Static(StaticNode::U64(_)) => { out.push(value); }
                _ => {}
            }
        }
        Builtin0::Ceil => {
            match &*value {
                BorrowedValue::Static(StaticNode::F64(f)) => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(f.ceil() as i64)))); }
                BorrowedValue::Static(StaticNode::I64(_)) | BorrowedValue::Static(StaticNode::U64(_)) => { out.push(value); }
                _ => {}
            }
        }
        Builtin0::Round => {
            match &*value {
                BorrowedValue::Static(StaticNode::F64(f)) => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(f.round() as i64)))); }
                BorrowedValue::Static(StaticNode::I64(_)) | BorrowedValue::Static(StaticNode::U64(_)) => { out.push(value); }
                _ => {}
            }
        }
        Builtin0::Sqrt => {
            if let Some((f, _)) = to_f64(&value) { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::F64(f.sqrt())))); }
        }
        Builtin0::Fabs => {
            match &*value {
                BorrowedValue::Static(StaticNode::I64(i)) => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(i.abs())))); }
                BorrowedValue::Static(StaticNode::F64(f)) => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::F64(f.abs())))); }
                BorrowedValue::Static(StaticNode::U64(_)) => { out.push(value); }
                _ => {}
            }
        }
        Builtin0::Nan => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::F64(f64::NAN)))); }
        Builtin0::Infinite => { out.push(Cow::Owned(BorrowedValue::Static(StaticNode::F64(f64::INFINITY)))); }
        Builtin0::Isinfinite => {
            let r = to_f64(&value).map_or(false, |(f, _)| f.is_infinite());
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(r))));
        }
        Builtin0::Isnan => {
            let r = to_f64(&value).map_or(false, |(f, _)| f.is_nan());
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(r))));
        }
        Builtin0::Isnormal => {
            let r = to_f64(&value).map_or(false, |(f, _)| f.is_normal());
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(r))));
        }
        Builtin0::Recurse => {
            match &value {
                Cow::Borrowed(b_val) => { recurse_values(b_val, out); }
                Cow::Owned(_) => {
                    let owned = value.into_owned();
                    out.push(Cow::Owned(clone_value(&owned)));
                    recurse_owned(owned, out);
                }
            }
        }
    }
}

// ─── builtin1 (one-arg) ────────────────────────────────────────────────────────

#[inline(never)]
fn exec_builtin1<'a>(b: &Builtin1, arg: &Literal, value: Cow<'a, BorrowedValue<'a>>, out: &mut Vec<Cow<'a, BorrowedValue<'a>>>) {
    match b {
        Builtin1::Has => {
            let r = match (&*value, arg) {
                (BorrowedValue::Object(obj), Literal::String(k)) => obj.contains_key(k.as_str()),
                (BorrowedValue::Array(arr), Literal::Int(i)) => { let idx = if *i < 0 { arr.len() as i64 + i } else { *i }; idx >= 0 && (idx as usize) < arr.len() }
                _ => false,
            };
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(r))));
        }
        Builtin1::Startswith => {
            if let (BorrowedValue::String(s), Literal::String(prefix)) = (&*value, arg) {
                out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(s.as_ref().starts_with(prefix.as_str())))));
            }
        }
        Builtin1::Endswith => {
            if let (BorrowedValue::String(s), Literal::String(suffix)) = (&*value, arg) {
                out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(s.as_ref().ends_with(suffix.as_str())))));
            }
        }
        Builtin1::Contains => {
            match (&*value, arg) {
                (BorrowedValue::String(s), Literal::String(sub)) => {
                    out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(s.as_ref().contains(sub.as_str())))));
                }
                (val, _) => {
                    let arg_val = literal_to_value(arg);
                    out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(value_contains(val, &arg_val)))));
                }
            }
        }
        Builtin1::Inside => {
            let arg_val = literal_to_value(arg);
            out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Bool(value_contains(&arg_val, &value)))));
        }
        Builtin1::Split => {
            if let (BorrowedValue::String(s), Literal::String(sep)) = (&*value, arg) {
                let parts: Vec<BorrowedValue> = s.as_ref().split(sep.as_str()).map(|p| BorrowedValue::String(Cow::Owned(p.to_string()))).collect();
                out.push(Cow::Owned(make_array(parts)));
            }
        }
        Builtin1::Join => {
            if let (BorrowedValue::Array(arr), Literal::String(sep)) = (&*value, arg) {
                let strings: Vec<String> = arr.iter().filter_map(|v| match v {
                    BorrowedValue::String(s) => Some(s.as_ref().to_string()),
                    BorrowedValue::Static(StaticNode::I64(i)) => Some(i.to_string()),
                    BorrowedValue::Static(StaticNode::U64(u)) => Some(u.to_string()),
                    BorrowedValue::Static(StaticNode::F64(f)) => Some(f.to_string()),
                    BorrowedValue::Static(StaticNode::Bool(b)) => Some(if *b { "true" } else { "false" }.to_string()),
                    BorrowedValue::Static(StaticNode::Null) => None,
                    _ => Some(value_to_json_string(v)),
                }).collect();
                out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(strings.join(sep.as_str())))));
            }
        }
        Builtin1::Ltrimstr => {
            if let (BorrowedValue::String(s), Literal::String(prefix)) = (&*value, arg) {
                let result = s.as_ref().strip_prefix(prefix.as_str()).unwrap_or(s.as_ref());
                out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(result.to_string()))));
            } else { out.push(value); }
        }
        Builtin1::Rtrimstr => {
            if let (BorrowedValue::String(s), Literal::String(suffix)) = (&*value, arg) {
                let result = s.as_ref().strip_suffix(suffix.as_str()).unwrap_or(s.as_ref());
                out.push(Cow::Owned(BorrowedValue::String(Cow::Owned(result.to_string()))));
            } else { out.push(value); }
        }
        Builtin1::FlattenDepth => {
            if let (BorrowedValue::Array(arr), Literal::Int(depth)) = (&*value, arg) {
                out.push(Cow::Owned(make_array(flatten_array(arr, Some(*depth as u64)))));
            }
        }
        Builtin1::Index => {
            match (&*value, arg) {
                (BorrowedValue::String(s), Literal::String(sub)) => {
                    match s.as_ref().find(sub.as_str()) {
                        Some(pos) => { let cp = s.as_ref()[..pos].chars().count() as i64; out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(cp)))); }
                        None => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null))),
                    }
                }
                (BorrowedValue::Array(arr), _) => {
                    let needle = literal_to_value(arg);
                    match arr.iter().position(|v| values_equal(v, &needle)) {
                        Some(pos) => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(pos as i64)))),
                        None => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null))),
                    }
                }
                _ => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null))),
            }
        }
        Builtin1::Rindex => {
            match (&*value, arg) {
                (BorrowedValue::String(s), Literal::String(sub)) => {
                    match s.as_ref().rfind(sub.as_str()) {
                        Some(pos) => { let cp = s.as_ref()[..pos].chars().count() as i64; out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(cp)))); }
                        None => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null))),
                    }
                }
                (BorrowedValue::Array(arr), _) => {
                    let needle = literal_to_value(arg);
                    let mut found = None;
                    for (i, v) in arr.iter().enumerate() { if values_equal(v, &needle) { found = Some(i); } }
                    match found {
                        Some(pos) => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::I64(pos as i64)))),
                        None => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null))),
                    }
                }
                _ => out.push(Cow::Owned(BorrowedValue::Static(StaticNode::Null))),
            }
        }
        Builtin1::Indices => {
            match (&*value, arg) {
                (BorrowedValue::String(s), Literal::String(sub)) => {
                    let mut positions = Vec::new();
                    let haystack = s.as_ref();
                    let needle = sub.as_str();
                    if !needle.is_empty() {
                        let mut start = 0;
                        while let Some(pos) = haystack[start..].find(needle) {
                            let abs_pos = start + pos;
                            positions.push(BorrowedValue::Static(StaticNode::I64(haystack[..abs_pos].chars().count() as i64)));
                            start = abs_pos + 1;
                        }
                    }
                    out.push(Cow::Owned(make_array(positions)));
                }
                (BorrowedValue::Array(arr), _) => {
                    let needle = literal_to_value(arg);
                    let positions: Vec<BorrowedValue> = arr.iter().enumerate()
                        .filter(|(_, v)| values_equal(v, &needle))
                        .map(|(i, _)| BorrowedValue::Static(StaticNode::I64(i as i64)))
                        .collect();
                    out.push(Cow::Owned(make_array(positions)));
                }
                _ => out.push(Cow::Owned(make_array(Vec::new()))),
            }
        }
        Builtin1::Limit => {
            if let Literal::Int(n) = arg {
                if let BorrowedValue::Array(arr) = &*value {
                    let taken: Vec<BorrowedValue> = arr.iter().take(*n as usize).map(clone_value).collect();
                    out.push(Cow::Owned(make_array(taken)));
                }
            }
        }
    }
}
