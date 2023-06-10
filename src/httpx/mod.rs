mod errors;
mod request;

pub use errors::FlaskError;

use crate::combinators::*;

use http::Version;
use nom::IResult;
use std::net::TcpStream;
use std::io::{
    BufReader,
    prelude::*
};

pub use request::read_http_request;

struct Header<'b> {
    key: &'b str,
    value: &'b str,
}

const CONTENT_LENGTH_HEADER: &str = "content-length";

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

fn read_buffered_line(reader: &mut BufReader<TcpStream>) -> Result<String, FlaskError> {
    let mut line: String = String::from("");
    match reader.read_line(&mut line) {
        Ok(num_bytes) => {
            if num_bytes != line.len() {
                let msg = format!("Error in request line byte count");
                let flask_err = FlaskError::InternalServerError(msg);
                Err(flask_err)
            } else {
                Ok(line)
            }
        },
        Err(buf_err) => {
            let msg = format!("Error reading buffered request line: {}", buf_err);
            let flask_err = FlaskError::ClientClosedRequest(msg);
            Err(flask_err)
        }
    }
}

fn read_header(line: &str) -> Result<Header, FlaskError> {
    let (line, key) = http_header_name(line).unwrap();
    let (line, _) = colon(line).unwrap();
    let (line, _) = space(line).unwrap();
    let (line, value) = header_value(line).unwrap();
    match crlf(line) {
      Ok(_) => Ok( Header {key: key, value: value} ),
      Err(_) => Err( FlaskError::BadRequest("Malformed Header: no terminating CRLF".to_string()) )
    }
  }
