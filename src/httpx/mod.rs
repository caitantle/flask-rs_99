mod errors;
mod request;
mod response;


// These function are the modules public interface
pub use errors::FlaskError;
pub use request::read_http_request;
pub use response::read_http_response;

use http::Version;
use nom::character::is_alphanumeric;
use std::str::{self, from_utf8};

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

fn get_http_version(ver_str: &str) -> Result<Version, FlaskError> {
    match ver_str {
        "1.1" => Ok( Version::HTTP_11 ),
        ver @ "0.9" | ver @ "1.0" | ver @ "2.0" | ver @ "3.0" => {
            let fmt_msg = format!("Unsupported HTTP version {}", ver);
            let msg = String::from(fmt_msg);
            let err = FlaskError::NotImplemented(msg);
            Err(err)
        },
        ver @ _ => {
            let fmt_msg = format!("Unknown HTTP version {}", ver);
            let msg = String::from(fmt_msg);
            let err = FlaskError::BadRequest(msg);
            Err(err)
        }
    }
}
