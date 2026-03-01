use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, char, digit1, multispace0},
    combinator::{map, map_res, opt, recognize},
    multi::{many1, separated_list1, many0},
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

// Represents filters operation in a jq-style query
#[derive(Debug, Clone)]
pub enum RustyFilter {
    Identity,
    Field(String),
    Index(i32),
    Iterator,
    Object(Vec<(String, Vec<RustyFilter>)>),
    Select(Vec<RustyFilter>, CompareOp, Literal),
}

fn parse_dot(input: &str) -> IResult<&str, &str> {
    tag(".")(input)
}

fn parse_word(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alphanumeric1, tag("_"))),
        opt(recognize(many0(alt((alphanumeric1, tag("_"), tag("-"))))))
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
    map(parse_dot, |_| RustyFilter::Identity)(input)
}

fn parse_key_value_pair(input: &str) -> IResult<&str, (String, Vec<RustyFilter>)> {
    map(
        separated_pair(
            parse_word,
            delimited(multispace0, char(':'), multispace0),
            parse_query,
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

fn parse_literal(input: &str) -> IResult<&str, Literal> {
    alt((
        map(tag("true"), |_| Literal::Bool(true)),
        map(tag("false"), |_| Literal::Bool(false)),
        map(tag("null"), |_| Literal::Null),
        map(
            delimited(char('"'), recognize(many0(alt((alphanumeric1, tag("_"), tag("-"), tag(" "))))), char('"')),
            |s: &str| Literal::String(s.to_string())
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

// boolean selection filter
fn parse_select(input: &str) -> IResult<&str, RustyFilter> {
    map(
        delimited(
            tag("select("),
            tuple((
                delimited(multispace0, parse_query, multispace0),
                parse_compare_op,
                delimited(multispace0, parse_literal, multispace0)
            )),
            char(')')
        ),
        |(path_filters, op, literal)| RustyFilter::Select(path_filters, op, literal)
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

// parses a full jq-style query string into a list of RustyFilter
pub fn parse_query(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    // many1 loops until it can't parse anymore
    many1(
        preceded(
            opt(delimited(multispace0, char('|'), multispace0)), 
            parse_single_filter
        )
    )(input)
}