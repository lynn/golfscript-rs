use nom::branch::alt;
use nom::bytes::complete::take;
use nom::bytes::complete::{take_while, take_while1, take_while_m_n};
use nom::character::{is_alphabetic, is_digit};
use nom::combinator::{consumed, recognize};
use nom::multi::many0;
use nom::sequence::{delimited, pair};
use nom::IResult;

#[derive(Clone, Debug)]
pub enum Gtoken<'a> {
    Symbol(&'a [u8]),             // [a-zA-Z_][a-zA-Z0-9_]* or final .
    SingleQuotedString(&'a [u8]), // '(?:\\.|[^'])*'?
    DoubleQuotedString(&'a [u8]), // "(?:\\.|[^"])*"?
    IntLiteral(&'a [u8]),         // -?[0-9]+
    Comment(&'a [u8]),            // #[^\n\r]*
    Block(Vec<Gtoken<'a>>, &'a [u8]),
}

impl<'a> Gtoken<'a> {
    pub fn lexeme(&self) -> &'a [u8] {
        match self {
            &Gtoken::Symbol(s)
            | &Gtoken::SingleQuotedString(s)
            | &Gtoken::DoubleQuotedString(s)
            | &Gtoken::IntLiteral(s)
            | &Gtoken::Comment(s)
            | &Gtoken::Block(_, s) => s,
        }
    }
}

fn single<'a, Error: nom::error::ParseError<&'a [u8]>>(
    b: u8,
) -> impl Fn(&'a [u8]) -> IResult<&'a [u8], &'a [u8], Error> {
    take_while_m_n(1, 1, move |c| c == b)
}

fn parse_identifier(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let head = take_while_m_n(1, 1, |c| is_alphabetic(c) || c == b'_');
    let tail = take_while(|c| is_alphabetic(c) || is_digit(c) || c == b'_');
    let (i, s) = recognize(pair(head, tail))(i)?;
    Ok((i, Gtoken::Symbol(s)))
}

fn parse_string(delimiter: u8, i: &[u8]) -> IResult<&[u8], &[u8]> {
    let inner = alt((
        recognize(pair(single(b'\\'), take(1usize))),
        take_while_m_n(1, 1, |c| c != delimiter),
    ));
    recognize(delimited(
        single(delimiter),
        many0(inner),
        single(delimiter),
    ))(i)
}

fn parse_single_quoted_string(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let (i, s) = parse_string(b'\'', i)?;
    Ok((i, Gtoken::SingleQuotedString(s)))
}

fn parse_double_quoted_string(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let (i, s) = parse_string(b'"', i)?;
    Ok((i, Gtoken::DoubleQuotedString(s)))
}

fn parse_int_literal(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let (i, s) = recognize(pair(
        take_while_m_n(0, 1, |b| b == b'-'),
        take_while1(is_digit),
    ))(i)?;
    Ok((i, Gtoken::IntLiteral(s)))
}

fn parse_comment(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let (i, s) = recognize(pair(single(b'#'), take_while(|b| b != b'\r' && b != b'\n')))(i)?;
    Ok((i, Gtoken::Comment(s)))
}

fn parse_block(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let (i, _) = single(b'{')(i)?;
    let (i, (src, tokens)) = consumed(parse_code)(i)?;
    let (i, _) = single(b'}')(i)?;
    Ok((i, Gtoken::Block(tokens, src)))
}

fn parse_symbol(i: &[u8]) -> IResult<&[u8], Gtoken> {
    let (i, s) = take_while_m_n(1, 1, |b| b != b'{' && b != b'}' && b != b'"' && b != b'\'')(i)?;
    Ok((i, Gtoken::Symbol(s)))
}

pub fn parse_token(i: &[u8]) -> IResult<&[u8], Gtoken> {
    alt((
        parse_identifier,
        parse_single_quoted_string,
        parse_double_quoted_string,
        parse_int_literal,
        parse_comment,
        parse_block,
        parse_symbol,
    ))(i)
}

pub fn parse_code(i: &[u8]) -> IResult<&[u8], Vec<Gtoken>> {
    many0(parse_token)(i)
}
