use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, char, digit1, multispace0},
    combinator::{map, map_res, opt, recognize},
    multi::{separated_list1, many0},
    sequence::{delimited, pair, preceded, separated_pair},
    IResult,
};

// Represents filters operation in a jq-style query
#[derive(Debug, Clone)]
pub enum RustyFilter {
    Identity,
    Select(String),
    Index(i32),
    Iterator,
    Object(Vec<(String, Vec<RustyFilter>)>),
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
fn parse_select(input: &str) -> IResult<&str, RustyFilter> {
    map(
        preceded(
            parse_dot, 
            parse_word
        ), 
        |s: &str| RustyFilter::Select(s.to_string())
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

// parses any single filter token
fn parse_single_filter(input: &str) -> IResult<&str, RustyFilter> {
    alt((
        parse_iterator,
        parse_index,
        parse_select,
        parse_object,
        parse_identity
    ))(input)
}

// parses a full jq-style query string into a list of RustyFilter
pub fn parse_query(input: &str) -> IResult<&str, Vec<RustyFilter>> {
    separated_list1(
        delimited(multispace0, char('|'), multispace0), 
        parse_single_filter
    )(input)
}