use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::digit1;
use nom::combinator::{map, recognize, opt, value};
use nom::IResult;
use nom::sequence::{delimited, pair, preceded};

#[derive(Debug, PartialEq, Clone)]
pub enum JrFilter {
    Identity,
    Select(String),
    Index(isize),
}

fn is_key_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn parse_identity(input: &str) -> IResult<&str, JrFilter> {
    alt((
        map(
            delimited(
                tag(".["),
                recognize(pair(opt(tag("-")), digit1)),
                tag("]")
            ),
            |s: &str| JrFilter::Index(s.parse().unwrap()),
        ),

        map(
            preceded(tag("."), take_while1(is_key_char)),
            |s: &str| JrFilter::Select(s.to_string()),
        ),

        value(JrFilter::Identity, tag("."))
    ))(input)
}