use nom::{
    branch::alt,
    bytes::complete::{tag, take_while},
    character::complete::{alpha1, char, digit1, multispace0},
    combinator::{map, map_res, opt, recognize},
    multi::{many1, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
    IResult,
};

#[derive(Debug, Clone, PartialEq)]
pub enum CompareOp {
    Eq,  // ==
    Neq, // !=
    Gt,  // >
    Lt,  // <
    Gte, // >=
    Lte, // <=
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Path(Vec<RustyFilter>),
}

#[derive(Debug, Clone)]
pub enum Condition {
    Comparison(Vec<RustyFilter>, CompareOp, Expr),
    BoolPath(Vec<RustyFilter>),
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone)]
pub enum Builtin0 {
    Length, Keys, KeysUnsorted, Values, Type,
    Reverse, Sort, Flatten, Add, Min, Max, Unique,
    First, Last, Not, Empty,
    Tostring, Tonumber,
    ToEntries, FromEntries,
    AsciiDowncase, AsciiUpcase,
    Tojson, Fromjson,
    Explode, Implode,
    Floor, Ceil, Round, Sqrt, Fabs,
    Nan, Infinite, Isinfinite, Isnan, Isnormal,
    Recurse,
}

#[derive(Debug, Clone)]
pub enum Builtin1 {
    Has, Startswith, Endswith, Contains, Inside,
    Split, Join, Ltrimstr, Rtrimstr,
    FlattenDepth,
    Index, Rindex, Indices,
    Limit,
}

// Represents filters operation in a jq-style query
#[derive(Debug, Clone)]
pub enum RustyFilter {
    Identity,
    Field(String),
    Index(i32),
    Iterator,
    Object(Vec<(String, Vec<RustyFilter>)>),
    Select(Condition),
    Comma(Vec<Vec<RustyFilter>>),
    Arithmetic(Vec<RustyFilter>, ArithOp, Vec<RustyFilter>),
    LiteralValue(Literal),
    Builtin0(Builtin0),
    Builtin1(Builtin1, Literal),
    RecurseDescent,
    Slice(Option<i64>, Option<i64>),
}

// keyword parser with word-boundary check
fn parse_keyword<'a>(kw: &'static str) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    move |input: &'a str| {
        let (rest, matched) = tag(kw)(input)?;
        if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
            Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
        } else {
            Ok((rest, matched))
        }
    }
}

fn parse_dot(input: &str) -> IResult<&str, &str> {
    tag(".")(input)
}

fn parse_word(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        take_while(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
    ))(input)
}
fn parse_field(input: &str) -> IResult<&str, RustyFilter> {
    map(
        preceded(
            parse_dot,
            parse_word
        ),
        |s: &str| RustyFilter::Field(s.to_string())
    )(input)
}

fn parse_index(input: &str) -> IResult<&str, RustyFilter> {
    map(
        preceded(
            parse_dot,
            delimited(
                char('['),
                map_res(
                    recognize(pair(opt(char('-')), digit1)),
                    |s: &str| s.parse::<i32>()
                ),
                char(']')
            )
        ),
        RustyFilter::Index
    )(input)
}

fn parse_slice(input: &str) -> IResult<&str, RustyFilter> {
    preceded(
        parse_dot,
        delimited(
            char('['),
            map(
                tuple((
                    opt(map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| s.parse::<i64>())),
                    char(':'),
                    opt(map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| s.parse::<i64>())),
                )),
                |(start, _, end)| RustyFilter::Slice(start, end),
            ),
            char(']'),
        ),
    )(input)
}

fn parse_iterator(input: &str) -> IResult<&str, RustyFilter> {
    map(
        preceded(parse_dot, tag("[]")),
        |_| RustyFilter::Iterator
    )(input)
}

fn parse_recursive_descent(input: &str) -> IResult<&str, RustyFilter> {
    map(tag(".."), |_| RustyFilter::RecurseDescent)(input)
}

fn parse_identity(input: &str) -> IResult<&str, RustyFilter> {
    let (rest, _) = parse_dot(input)?;
    if rest.starts_with('.') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, RustyFilter::Identity))
    }
}

fn parse_key_value_pair(input: &str) -> IResult<&str, (String, Vec<RustyFilter>)> {
    map(
        separated_pair(
            parse_word,
            delimited(multispace0, char(':'), multispace0),
            parse_pipeline,
        ),
        |(k, v)| (k.to_string(), v),
    )(input)
}

// object construction
fn parse_object(input: &str) -> IResult<&str, RustyFilter> {
    map(
        delimited(
            char('{'),
            delimited(
                multispace0,
                separated_list1(
                    delimited(multispace0, char(','), multispace0),
                    parse_key_value_pair
                ),
                multispace0
            ),
            char('}')
        ),
        RustyFilter::Object
    )(input)
}

fn parse_compare_op(input: &str) -> IResult<&str, CompareOp> {
    alt((
        map(tag("=="), |_| CompareOp::Eq),
        map(tag("!="), |_| CompareOp::Neq),
        map(tag(">="), |_| CompareOp::Gte),
        map(tag("<="), |_| CompareOp::Lte),
        map(tag(">"), |_| CompareOp::Gt),
        map(tag("<"), |_| CompareOp::Lt),
    ))(input)
}

fn parse_string_contents(input: &str) -> IResult<&str, String> {
    let mut result = String::new();
    let mut chars = input.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((&input[i..], result)),
            '\\' => match chars.next() {
                Some((_, '"'))  => result.push('"'),
                Some((_, '\\')) => result.push('\\'),
                Some((_, 'n'))  => result.push('\n'),
                Some((_, 't'))  => result.push('\t'),
                Some((_, 'r'))  => result.push('\r'),
                Some((_, '/'))  => result.push('/'),
                Some((_, 'u'))  => {
                    let mut hex = String::with_capacity(4);
                    for _ in 0..4 {
                        match chars.next() {
                            Some((_, c)) if c.is_ascii_hexdigit() => hex.push(c),
                            _ => return Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char))),
                        }
                    }
                    let code = u32::from_str_radix(&hex, 16)
                        .map_err(|_| nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char)))?;
                    // handle UTF-16 surrogate pairs
                    let code = if (0xD800..=0xDBFF).contains(&code) {
                        match (chars.next(), chars.next()) {
                            (Some((_, '\\')), Some((_, 'u'))) => {}
                            _ => return Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char))),
                        }
                        let mut hex2 = String::with_capacity(4);
                        for _ in 0..4 {
                            match chars.next() {
                                Some((_, c)) if c.is_ascii_hexdigit() => hex2.push(c),
                                _ => return Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char))),
                            }
                        }
                        let low = u32::from_str_radix(&hex2, 16)
                            .map_err(|_| nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char)))?;
                        if !(0xDC00..=0xDFFF).contains(&low) {
                            return Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char)));
                        }
                        0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00)
                    } else {
                        code
                    };
                    match char::from_u32(code) {
                        Some(c) => result.push(c),
                        None => return Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char))),
                    }
                },
                _ => return Err(nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char))),
            },
            _ => result.push(c),
        }
    }
    // reached end of input without closing quote
    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))
}

fn parse_literal(input: &str) -> IResult<&str, Literal> {
    alt((
        map(parse_keyword("true"), |_| Literal::Bool(true)),
        map(parse_keyword("false"), |_| Literal::Bool(false)),
        map(parse_keyword("null"), |_| Literal::Null),
        map(
            delimited(char('"'), parse_string_contents, char('"')),
            Literal::String
        ),
        map(
            map_res(
                recognize(tuple((opt(char('-')), digit1, char('.'), digit1))),
                |s: &str| s.parse::<f64>()
            ),
            Literal::Float
        ),
        map(
            map_res(recognize(pair(opt(char('-')), digit1)), |s: &str| s.parse::<i64>()),
            Literal::Int
        ),
    ))(input)
}

// parse an expression: literal or path
fn parse_expr(input: &str) -> IResult<&str, Expr> {
    alt((
        map(parse_literal, Expr::Literal),
        map(parse_pipeline, Expr::Path),
    ))(input)
}

// parse a comparison or bare boolean path
// path [op expr] [| not]
fn parse_comparison_or_bool_path(input: &str) -> IResult<&str, Condition> {
    let (rest, path) = parse_pipeline(input)?;
    // try to parse an operator + expression
    if let Ok((rest2, (_, op, _, expr, _))) = tuple((
        multispace0::<&str, nom::error::Error<&str>>,
        parse_compare_op,
        multispace0,
        parse_expr,
        multispace0,
    ))(rest) {
        // check for trailing `| not`
        if let Ok((rest3, _)) = tuple((
            multispace0::<&str, nom::error::Error<&str>>,
            char('|'),
            multispace0,
            parse_keyword("not"),
        ))(rest2) {
            Ok((rest3, Condition::Not(Box::new(Condition::Comparison(path, op, expr)))))
        } else {
            Ok((rest2, Condition::Comparison(path, op, expr)))
        }
    } else {
        // no operator — check for `| not` on bare path
        if let Ok((rest2, _)) = tuple((
            multispace0::<&str, nom::error::Error<&str>>,
            char('|'),
            multispace0,
            parse_keyword("not"),
        ))(rest) {
            Ok((rest2, Condition::Not(Box::new(Condition::BoolPath(path)))))
        } else {
            Ok((rest, Condition::BoolPath(path)))
        }
    }
}

// atom: parenthesized condition or comparison/bool
fn parse_condition_atom(input: &str) -> IResult<&str, Condition> {
    alt((
        delimited(
            pair(char('('), multispace0),
            parse_condition,
            pair(multispace0, char(')')),
        ),
        parse_comparison_or_bool_path,
    ))(input)
}

// not-expr: "not" atom | atom  (prefix not)
fn parse_condition_not(input: &str) -> IResult<&str, Condition> {
    alt((
        map(
            preceded(pair(parse_keyword("not"), multispace0), parse_condition_atom),
            |c| Condition::Not(Box::new(c)),
        ),
        parse_condition_atom,
    ))(input)
}

// and-expr: not-expr ("and" not-expr)*
fn parse_condition_and(input: &str) -> IResult<&str, Condition> {
    let (mut rest, mut left) = parse_condition_not(input)?;
    loop {
        let try_and = tuple((
            multispace0::<&str, nom::error::Error<&str>>,
            parse_keyword("and"),
            multispace0,
        ))(rest);
        match try_and {
            Ok((rest2, _)) => {
                let (rest3, right) = parse_condition_not(rest2)?;
                left = Condition::And(Box::new(left), Box::new(right));
                rest = rest3;
            }
            Err(_) => break,
        }
    }
    Ok((rest, left))
}

// condition: and-expr ("or" and-expr)*
fn parse_condition(input: &str) -> IResult<&str, Condition> {
    let (mut rest, mut left) = parse_condition_and(input)?;
    loop {
        let try_or = tuple((
            multispace0::<&str, nom::error::Error<&str>>,
            parse_keyword("or"),
            multispace0,
        ))(rest);
        match try_or {
            Ok((rest2, _)) => {
                let (rest3, right) = parse_condition_and(rest2)?;
                left = Condition::Or(Box::new(left), Box::new(right));
                rest = rest3;
            }
            Err(_) => break,
        }
    }
    Ok((rest, left))
}

// boolean selection filter
fn parse_select(input: &str) -> IResult<&str, RustyFilter> {
    map(
        delimited(
            tuple((tag("select"), multispace0, char('('))),
            delimited(multispace0, parse_condition, multispace0),
            char(')')
        ),
        RustyFilter::Select
    )(input)
}

// no-arg builtins
fn parse_builtin0(input: &str) -> IResult<&str, RustyFilter> {
    alt((
        alt((
            map(parse_keyword("length"), |_| RustyFilter::Builtin0(Builtin0::Length)),
            map(parse_keyword("keys_unsorted"), |_| RustyFilter::Builtin0(Builtin0::KeysUnsorted)),
            map(parse_keyword("keys"), |_| RustyFilter::Builtin0(Builtin0::Keys)),
            map(parse_keyword("values"), |_| RustyFilter::Builtin0(Builtin0::Values)),
            map(parse_keyword("type"), |_| RustyFilter::Builtin0(Builtin0::Type)),
            map(parse_keyword("reverse"), |_| RustyFilter::Builtin0(Builtin0::Reverse)),
            map(parse_keyword("sort"), |_| RustyFilter::Builtin0(Builtin0::Sort)),
            map(parse_keyword("flatten"), |_| RustyFilter::Builtin0(Builtin0::Flatten)),
            map(parse_keyword("add"), |_| RustyFilter::Builtin0(Builtin0::Add)),
            map(parse_keyword("min"), |_| RustyFilter::Builtin0(Builtin0::Min)),
            map(parse_keyword("max"), |_| RustyFilter::Builtin0(Builtin0::Max)),
            map(parse_keyword("unique"), |_| RustyFilter::Builtin0(Builtin0::Unique)),
            map(parse_keyword("first"), |_| RustyFilter::Builtin0(Builtin0::First)),
            map(parse_keyword("last"), |_| RustyFilter::Builtin0(Builtin0::Last)),
            map(parse_keyword("not"), |_| RustyFilter::Builtin0(Builtin0::Not)),
            map(parse_keyword("empty"), |_| RustyFilter::Builtin0(Builtin0::Empty)),
            map(parse_keyword("tostring"), |_| RustyFilter::Builtin0(Builtin0::Tostring)),
            map(parse_keyword("tonumber"), |_| RustyFilter::Builtin0(Builtin0::Tonumber)),
            map(parse_keyword("to_entries"), |_| RustyFilter::Builtin0(Builtin0::ToEntries)),
            map(parse_keyword("from_entries"), |_| RustyFilter::Builtin0(Builtin0::FromEntries)),
            map(parse_keyword("ascii_downcase"), |_| RustyFilter::Builtin0(Builtin0::AsciiDowncase)),
        )),
        alt((
            map(parse_keyword("ascii_upcase"), |_| RustyFilter::Builtin0(Builtin0::AsciiUpcase)),
            map(parse_keyword("tojson"), |_| RustyFilter::Builtin0(Builtin0::Tojson)),
            map(parse_keyword("fromjson"), |_| RustyFilter::Builtin0(Builtin0::Fromjson)),
            map(parse_keyword("explode"), |_| RustyFilter::Builtin0(Builtin0::Explode)),
            map(parse_keyword("implode"), |_| RustyFilter::Builtin0(Builtin0::Implode)),
            map(parse_keyword("floor"), |_| RustyFilter::Builtin0(Builtin0::Floor)),
            map(parse_keyword("ceil"), |_| RustyFilter::Builtin0(Builtin0::Ceil)),
            map(parse_keyword("round"), |_| RustyFilter::Builtin0(Builtin0::Round)),
            map(parse_keyword("sqrt"), |_| RustyFilter::Builtin0(Builtin0::Sqrt)),
            map(parse_keyword("fabs"), |_| RustyFilter::Builtin0(Builtin0::Fabs)),
            map(parse_keyword("nan"), |_| RustyFilter::Builtin0(Builtin0::Nan)),
            map(parse_keyword("infinite"), |_| RustyFilter::Builtin0(Builtin0::Infinite)),
            map(parse_keyword("isinfinite"), |_| RustyFilter::Builtin0(Builtin0::Isinfinite)),
            map(parse_keyword("isnan"), |_| RustyFilter::Builtin0(Builtin0::Isnan)),
            map(parse_keyword("isnormal"), |_| RustyFilter::Builtin0(Builtin0::Isnormal)),
            map(parse_keyword("recurse"), |_| RustyFilter::Builtin0(Builtin0::Recurse)),
        )),
    ))(input)
}

// helper: parse "keyword(" literal ")"
fn parse_builtin1_call<'a>(kw: &'static str, b: Builtin1) -> impl FnMut(&'a str) -> IResult<&'a str, RustyFilter> {
    move |input: &'a str| {
        let (rest, _) = parse_keyword(kw)(input)?;
        let (rest, _) = multispace0(rest)?;
        let (rest, _) = char('(')(rest)?;
        let (rest, _) = multispace0(rest)?;
        let (rest, lit) = parse_literal(rest)?;
        let (rest, _) = multispace0(rest)?;
        let (rest, _) = char(')')(rest)?;
        Ok((rest, RustyFilter::Builtin1(b.clone(), lit)))
    }
}

// 1-arg builtins
fn parse_builtin1(input: &str) -> IResult<&str, RustyFilter> {
    alt((
        alt((
            parse_builtin1_call("has", Builtin1::Has),
            parse_builtin1_call("startswith", Builtin1::Startswith),
            parse_builtin1_call("endswith", Builtin1::Endswith),
            parse_builtin1_call("contains", Builtin1::Contains),
            parse_builtin1_call("inside", Builtin1::Inside),
            parse_builtin1_call("split", Builtin1::Split),
            parse_builtin1_call("join", Builtin1::Join),
            parse_builtin1_call("ltrimstr", Builtin1::Ltrimstr),
            parse_builtin1_call("rtrimstr", Builtin1::Rtrimstr),
        )),
        alt((
            parse_builtin1_call("flatten", Builtin1::FlattenDepth),
            parse_builtin1_call("indices", Builtin1::Indices),
            parse_builtin1_call("index", Builtin1::Index),
            parse_builtin1_call("rindex", Builtin1::Rindex),
            parse_builtin1_call("limit", Builtin1::Limit),
        )),
    ))(input)
}

// parses any single filter token
fn parse_single_filter(input: &str) -> IResult<&str, RustyFilter> {
    alt((
        parse_select,
        parse_builtin1,
        parse_builtin0,
        parse_iterator,
        parse_slice,
        parse_index,
        parse_recursive_descent,
        parse_field,
        parse_object,
        parse_identity
    ))(input)
}

// parses a pipeline: sequence of filters with optional | separators (no commas)
// used internally by object values, select conditions, etc.
// arithmetic-aware so `.price + .tax` works inside objects and select
fn parse_pipeline(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    let (rest, segments) = many1(
        preceded(
            opt(delimited(multispace0, char('|'), multispace0)),
            parse_add_sub
        )
    )(input)?;
    Ok((rest, segments.into_iter().flatten().collect()))
}

// arithmetic atom: a chain of single filters, or a bare literal (for `.price + 10`)
fn parse_arith_atom(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    alt((
        many1(parse_single_filter),
        map(parse_literal, |lit| vec![RustyFilter::LiteralValue(lit)]),
    ))(input)
}

// mul/div/mod: arith_atom ((*|/|%) arith_atom)*
fn parse_mul_div(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    let (mut rest, mut left) = parse_arith_atom(input)?;
    loop {
        let try_op = tuple((
            multispace0::<&str, nom::error::Error<&str>>,
            alt((
                map(char('*'), |_| ArithOp::Mul),
                map(char('/'), |_| ArithOp::Div),
                map(char('%'), |_| ArithOp::Mod),
            )),
            multispace0,
        ))(rest);
        match try_op {
            Ok((rest2, (_, op, _))) => {
                let (rest3, right) = parse_arith_atom(rest2)?;
                left = vec![RustyFilter::Arithmetic(left, op, right)];
                rest = rest3;
            }
            Err(_) => break,
        }
    }
    Ok((rest, left))
}

// add/sub: mul_div ((+|-) mul_div)*
// `-` requires surrounding spaces to avoid ambiguity with hyphenated field names
fn parse_add_sub(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    let (mut rest, mut left) = parse_mul_div(input)?;
    loop {
        // try `+` with optional whitespace
        let try_add = tuple((
            multispace0::<&str, nom::error::Error<&str>>,
            map(char('+'), |_| ArithOp::Add),
            multispace0,
        ))(rest);
        if let Ok((rest2, (_, op, _))) = try_add {
            let (rest3, right) = parse_mul_div(rest2)?;
            left = vec![RustyFilter::Arithmetic(left, op, right)];
            rest = rest3;
            continue;
        }
        // try `-` only when preceded by whitespace (to avoid `.a-b` field names)
        if rest.starts_with(' ') || rest.starts_with('\t') || rest.starts_with('\n') {
            let try_sub = tuple((
                multispace0::<&str, nom::error::Error<&str>>,
                map(char('-'), |_| ArithOp::Sub),
                multispace0,
            ))(rest);
            if let Ok((rest2, (_, op, _))) = try_sub {
                let (rest3, right) = parse_mul_div(rest2)?;
                left = vec![RustyFilter::Arithmetic(left, op, right)];
                rest = rest3;
                continue;
            }
        }
        break;
    }
    Ok((rest, left))
}

// parses a comma-separated segment: arith_expr (, arith_expr)*
// if single expression, returns it as-is; if multiple, wraps in Comma
fn parse_comma_segment(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    let (rest, chains) = separated_list1(
        delimited(multispace0, char(','), multispace0),
        parse_add_sub
    )(input)?;
    if chains.len() == 1 {
        Ok((rest, chains.into_iter().next().unwrap()))
    } else {
        Ok((rest, vec![RustyFilter::Comma(chains)]))
    }
}

// top-level query: pipe-separated comma segments
// pipe has lowest precedence, comma has higher precedence
pub fn parse_query(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    let (rest, segments) = separated_list1(
        delimited(multispace0, char('|'), multispace0),
        parse_comma_segment
    )(input)?;
    Ok((rest, segments.into_iter().flatten().collect()))
}
