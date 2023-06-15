use nom::branch::alt;
use nom::bytes::complete::{is_a, is_not, take, take_until1};
use nom::bytes::streaming::{tag, tag_no_case, take_while};
use nom::character::{is_alphanumeric, is_space};
use nom::combinator::map_res;
use nom::error::ErrorKind;
use nom::Needed::Size;
use std::str::{self, from_utf8};

use nom::Err;
use nom::Err::Incomplete;
use nom::error::Error;
use nom::IResult;

// ***************************************************************************
// scalar combinators
// ***************************************************************************

pub fn crlf(i: &str) -> IResult<&str, &str> {
  tag("\r\n")(i)
}

pub fn colon(i: &str) -> IResult<&str, &str> {
  tag(":")(i)
}

// pub fn slash(i: &str) -> IResult<&str, &str> {
//   tag("/")(i)
// }

pub fn space(i: &str) -> IResult<&str, &str> {
  tag(" ")(i)
}

// ***************************************************************************
// repeated combinators
// ***************************************************************************
pub fn spaces(i: &str) -> IResult<&str, &str> {
  is_a(" ")(i)
}

pub fn digits(i: &str) -> IResult<&str, &str> {
  is_a("0123456789")(i)
}

pub fn to_space(s: &str) -> IResult<&str, &str> {
    is_not(" ")(s)
}

pub fn take_until(s: &str) -> IResult<&str, &str> {
  take_until1("/r")(s)
}

// ***************************************************************************
// classifiers
// ***************************************************************************
pub fn is_digit_char(ch: char) -> bool {
  let ch_u8 = ch as u8;
  b"0123456789".contains(&ch_u8)
}

pub fn number(s: &str) -> IResult<&str, &str> {
  take_while(is_digit_char)(s)
}

// ***************************************************************************
// http related combinators
// ***************************************************************************
// pub fn http(i: &str) -> IResult<&str, &str> {
//   tag_no_case("HTTP")(i)
// }

pub fn http_method(s: &str) -> IResult<&str, &str> {
  alt((
    tag("CONNECT"),
    tag("DELETE"),
    tag("GET"),
    tag("HEAD"),
    tag("OPTIONS"),
    tag("PATCH"),
    tag("POST"),
    tag("PUT"),
    tag("TRACE") 
  ))(s)
}

// pub fn http_version(s: &str) -> IResult<&str, &str> {
//   take(3usize)(s)
// }

pub fn http_version(s: &str) -> IResult<&str, &str> {
  let result = tag_no_case("HTTP/")(s);
  match result {
    Ok((s2, _)) => take(3usize)(s2),
    Err(_) => result
  }
}

fn is_http_header_name_char(ch: char) -> bool {
  let ch_u8 = ch as u8;
  is_alphanumeric(ch_u8) ||
  b"!#$%&'*+-.^_`|~".contains(&ch_u8)
}

pub fn http_header_name(s: &str) -> IResult<&str, &str> {
  take_while(is_http_header_name_char)(s)
}

// allows ISO-8859-1 characters in header values
// this is allowed in RFC 2616 but not in rfc7230
#[cfg(  feature = "tolerant-http1-parser")]
pub fn is_header_value_char(ch: char) -> bool {
  let ch_u8 = ch as u8;
  ch_u8 == 9 || (ch_u8 >= 32 && ch_u8 <= 126) || i >= 160
}

#[cfg(not(feature = "tolerant-http1-parser"))]
pub fn is_header_value_char(ch: char) -> bool {
  let ch_u8 = ch as u8;
  ch_u8 == 9 || (ch_u8 >= 32 && ch_u8 <= 126)
}

pub fn header_value(s: &str) -> IResult<&str, &str> {
  take_while(is_header_value_char)(s)
}


//#################################################################################################################
// test cases go below here
//#################################################################################################################
#[cfg(test)]
mod tests {
  extern crate rand;

  use super::*;


  #[test]
  fn test_crlf() {
    assert_eq!(crlf("\r\n"), Ok(("", "\r\n")));
    assert_eq!(crlf("\r\nWorld!"), Ok(("World!", "\r\n")));
    assert_eq!(crlf("\r\nHello\r\nWorld!"), Ok(("Hello\r\nWorld!", "\r\n")));
    assert_eq!(crlf("\r\n     "), Ok(("     ", "\r\n")));

    let resp = crlf("Something");
    let err = Err(Err::Error(Error::new("Something", ErrorKind::Tag)));
    assert_eq!(resp, err);

    let resp2 = crlf("Foo\r\nBar");
    let err2 = Err(Err::Error(Error::new("Foo\r\nBar", ErrorKind::Tag)));
    assert_eq!(resp2, err2); 

    // let resp3 = crlf("");
    // let err3 = Err(Incomplete(Size(2)));
    // assert_eq!(resp3, err3); 
  }

  #[test]
  fn test_space() {
    assert_eq!(space(" "), Ok(("", " ")));
    assert_eq!(space(" Hello"), Ok(("Hello", " ")));
    assert_eq!(space("      "), Ok(("     ", " ")));

    let resp = space("Rust World");
    let err = Err(Err::Error(Error::new("Rust World", ErrorKind::Tag)));
    assert_eq!(resp, err);
  }

  #[test]
  fn test_spaces() {
    assert_eq!(spaces(" "), Ok(("", " ")));
    assert_eq!(spaces("   "), Ok(("", "   ")));
    assert_eq!(spaces(" cat"), Ok(("cat", " ")));
    assert_eq!(spaces("   Hello"), Ok(("Hello", "   ")));
    assert_eq!(spaces("  "), Ok(("", "  ")));
    assert_eq!(spaces("  TREE  "), Ok(("TREE  ", "  ")));

    let resp = spaces("none");
    let err = Err(Err::Error(Error::new("none", ErrorKind::IsA)));
    assert_eq!(resp, err); 
  }

  #[test]
  fn test_digits() {
    assert_eq!(digits("7"), Ok(("", "7")));
    assert_eq!(digits("777"), Ok(("", "777")));
    assert_eq!(digits("123xxx456"), Ok(("xxx456", "123")));

    let resp = digits("car 5");
    let err = Err(Err::Error(Error::new("car 5", ErrorKind::IsA)));
    assert_eq!(resp, err);
  }

  // #[test]
  // fn test_http_tag() {
  //   assert_eq!(http("http"), Ok(("", "http")));
  //   assert_eq!(http("HTTP"), Ok(("", "HTTP")));
  //   assert_eq!(http("HttP"), Ok(("", "HttP")));
  //   assert_eq!(http("HttP\nparser"), Ok(("\nparser", "HttP")));
  // }

  #[test]
  fn test_http_method() {
    assert_eq!(http_method("POST foo bar baz"), Ok((" foo bar baz", "POST")));
    assert_eq!(http_method("HEAD foo bar baz"), Ok((" foo bar baz", "HEAD")));
    assert_eq!(http_method("GET foo bar baz"), Ok((" foo bar baz", "GET")));
  }
}

