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

fn parse_iterator(input: &str) -> IResult<&str, RustyFilter> {
    map(
        preceded(parse_dot, tag("[]")),
        |_| RustyFilter::Iterator
    )(input)
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
        map(parse_keyword_true, |_| Literal::Bool(true)),
        map(parse_keyword_false, |_| Literal::Bool(false)),
        map(parse_keyword_null, |_| Literal::Null),
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

// keyword parsers — ensure word boundary so `.and_field` isn't matched as `and`
fn parse_keyword_and(input: &str) -> IResult<&str, &str> {
    let (rest, matched) = tag("and")(input)?;
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, matched))
    }
}

fn parse_keyword_or(input: &str) -> IResult<&str, &str> {
    let (rest, matched) = tag("or")(input)?;
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, matched))
    }
}

fn parse_keyword_not(input: &str) -> IResult<&str, &str> {
    let (rest, matched) = tag("not")(input)?;
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, matched))
    }
}

fn parse_keyword_true(input: &str) -> IResult<&str, &str> {
    let (rest, matched) = tag("true")(input)?;
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, matched))
    }
}

fn parse_keyword_false(input: &str) -> IResult<&str, &str> {
    let (rest, matched) = tag("false")(input)?;
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, matched))
    }
}

fn parse_keyword_null(input: &str) -> IResult<&str, &str> {
    let (rest, matched) = tag("null")(input)?;
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
    } else {
        Ok((rest, matched))
    }
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
            parse_keyword_not,
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
            parse_keyword_not,
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
            preceded(pair(parse_keyword_not, multispace0), parse_condition_atom),
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
            parse_keyword_and,
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
            parse_keyword_or,
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

// parses any single filter token
fn parse_single_filter(input: &str) -> IResult<&str, RustyFilter> {
    alt((
        parse_select,
        parse_iterator,
        parse_index,
        parse_field,
        parse_object,
        parse_identity
    ))(input)
}

// parses a pipeline: sequence of filters with optional | separators (no commas)
// used internally by object values, select conditions, etc.
fn parse_pipeline(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    many1(
        preceded(
            opt(delimited(multispace0, char('|'), multispace0)),
            parse_single_filter
        )
    )(input)
}

// parses a chain of consecutive filters (no pipe, no comma)
fn parse_filter_chain(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    many1(parse_single_filter)(input)
}

// parses a comma-separated segment: chain (, chain)*
// if single chain, returns it as-is; if multiple, wraps in Comma
fn parse_comma_segment(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    let (rest, chains) = separated_list1(
        delimited(multispace0, char(','), multispace0),
        parse_filter_chain
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