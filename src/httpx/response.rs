use super::{
    errors::FlaskError,
    CONTENT_LENGTH_HEADER,
    get_http_version,
    read_buffered_line,
    read_header
  };
  
use crate::combinators::*;
 
#[derive(PartialEq, Debug)]
struct ResponseLine<'a> {
    status_code: &'a str,
    version: &'a str,
}

use http::{Response, StatusCode};
use http::response::Builder;
//   use nom::Parser;
use std::io::{
  BufReader,
  prelude::*
};
use std::net::TcpStream;

// named!( parse_response_line <ResponseLine>,
//   do_parse!(
//       http >> slash >> version: http_version >> opt!(spaces) >>
//       status_code: number >> spaces >> take_until1!("\r") >> crlf >>
//       (ResponseLine {status_code: status_code, version: version})
//   )
// );

fn parse_response_line(line: &str) -> Result<ResponseLine, FlaskError> {
  let (line, version): (&str, &str) = http_version(line).unwrap();
  let (line, _): (&str, &str) = spaces(line).unwrap();  // make me opt!  ??
  let (line, status_code): (&str, &str) = number(line).unwrap();
  let (line, _): (&str, &str) = spaces(line).unwrap();  // make me opt!  ??
  let (line, _): (&str, &str) = take_until(line).unwrap();
  match crlf(line) {
    Ok(_) => Ok(ResponseLine {status_code: status_code, version: version}),
    Err(_) => Err( FlaskError::BadRequest("Malformed Response Line: no terminating CRLF".to_string()) )
  }
}

fn _read_initial_request_line(reader: &mut BufReader<TcpStream>) -> Result<Builder, FlaskError> {
  let mut response = Response::builder();

  let mut line: String = String::from("");
  match reader.read_line(&mut line) {
      Ok(_) => {
          let resp_line: ResponseLine = match parse_response_line(line.as_str()) {
            Ok(parsed_line) => parsed_line,
            Err(_) => {
                let msg = format!("Malformed first line of response");
                return Err( FlaskError::BadRequest(msg) );
            }
          };

          let status_code_bytes = resp_line.status_code.as_bytes();
          let status_code = match StatusCode::from_bytes(status_code_bytes) {
              Ok(_status_code) => _status_code,
              Err(parse_err) => {
                  eprintln!("ERROR in response.rs calling StatusCode::from_bytes");
                  let msg: String = parse_err.to_string();
                  let flask_err = FlaskError::BadRequest(msg);
                  return Err(flask_err);
              }
          };
          let http_version = get_http_version(resp_line.version)?;

          response = response
              .status(status_code)
              .version(http_version);
      },
      Err(_) => {}
  }
  Ok(response)
}

fn _read_http_response(reader: &mut BufReader<TcpStream>) -> Result<Response<Vec<u8>>, FlaskError> {
  let mut response = _read_initial_request_line(reader)?;

  let content_length = {
      let mut content_length_mut = 0;
      loop {
          let line: String = read_buffered_line(reader)?;
          if line.as_str() == "\r\n" {
              break;
          }

          let header_line = match read_header(line.as_str()) {
            Ok(hl) => hl,
            Err(err) => {
                return Err( err );
            }
        };
          // println!("{:?}", line.as_bytes());

          if header_line.key.to_lowercase() == CONTENT_LENGTH_HEADER {
              match header_line.value.parse::<usize>() {
                  Ok(val) => content_length_mut = val,
                  Err(_) => {
                      let msg = format!("Invalid Content-Length: {}", header_line.value);
                      return Err( FlaskError::BadRequest(msg) );
                  }
              }
          }
          // println!("Key => {}", elem.key);
          response = response.header(header_line.key, header_line.value);
      }
      content_length_mut
  };


  let mut body = vec![0; content_length];
  match reader.read_exact(&mut body) {
      Ok(_) => {
          match response.body(body) {
              Ok(req) => Ok(req),
              Err(http_err) => {
                  eprintln!("ERROR reading response body from stream");
                  let msg: String = http_err.to_string();
                  let flask_err = FlaskError::ClientClosedRequest(msg);
                  Err(flask_err)
              }
          }
      },
      Err(http_err) => {
          let msg: String = http_err.to_string();
          let flask_err = FlaskError::BadRequest(msg);
          Err(flask_err) 
      }
  }
}

