mod request;
mod response;

pub use request::read_http_request;
pub use response::read_http_response;

use std::str::{self, from_utf8};
use nom::character::is_alphanumeric;

use crate::combinators::*;

struct Header<'b> {
    key: &'b str,
    value: &'b str,
}

fn is_token_char(i: u8) -> bool {
    is_alphanumeric(i) ||
    b"!#$%&'*+-.^_`|~".contains(&i)
  }

named!(token <&str>,
    map_res!(
        take_while!(is_token_char),
        from_utf8
    )
);

named!( http_version <&str>,
    map_res!(
        take!(3),
        from_utf8
    )
);

named!( to_space <&str>,
    map_res!(
        is_not!(" "),
        from_utf8
    )
);

named!( read_header <Header>,
    do_parse!(
        key: http_header_name >> colon >> space >> value: header_value >> crlf >>
        (Header {key: key, value: value})
    )
);
