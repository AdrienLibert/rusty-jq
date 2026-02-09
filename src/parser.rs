use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alphanumeric1, char, digit1, multispace0},
    combinator::{map, map_res, opt, recognize},
    multi::separated_list1,
    sequence::{delimited, pair, preceded},
    IResult,
};

#[derive(Debug, Clone)]
pub enum JrFilter {
    Identity,
    Select(String),
    Index(i32),
}


fn parse_dot(input: &str) -> IResult<&str, &str> {
    tag(".")(input)
}

fn parse_select(input: &str) -> IResult<&str, JrFilter> {
    map(
        preceded(
            parse_dot, 
            recognize(pair(
                alt((alphanumeric1, tag("_"))),
                opt(recognize(many_alphanumeric_underscore))
            ))
        ), 
        |s: &str| JrFilter::Select(s.to_string())
    )(input)
}

fn many_alphanumeric_underscore(input: &str) -> IResult<&str, &str> {
    recognize(nom::multi::many0(alt((alphanumeric1, tag("_")))))(input)
}

fn parse_index(input: &str) -> IResult<&str, JrFilter> {
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
        JrFilter::Index
    )(input)
}

fn parse_identity(input: &str) -> IResult<&str, JrFilter> {
    map(parse_dot, |_| JrFilter::Identity)(input)
}

fn parse_single_filter(input: &str) -> IResult<&str, JrFilter> {
    alt((
        parse_index,
        parse_select,
        parse_identity
    ))(input)
}

pub fn parse_query(input: &str) -> IResult<&str, Vec<JrFilter>> {
    separated_list1(
        delimited(multispace0, char('|'), multispace0), 
        parse_single_filter
    )(input)
}