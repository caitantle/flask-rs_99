use super::{
  errors::FlaskError,
  CONTENT_LENGTH_HEADER,
  get_http_version,
  read_buffered_line,
  read_header
};

use crate::combinators::*;

use http::Request;
use http::request::Builder;
use nom::Parser;
use std::io::{
  BufReader,
  prelude::*
};
use std::net::TcpStream;

#[derive(PartialEq, Debug)]
struct RequestLine<'a> {
    method: &'a str,
    target: &'a str,
    version: &'a str,
}

fn parse_request_line(line: &str) -> Result<RequestLine, FlaskError> {
    let (line, method): (&str, &str) = http_method(line).unwrap();
    let (line, _): (&str, &str) = spaces(line).unwrap();  // make me opt!  ??
    let (line, target): (&str, &str) = to_space(line).unwrap();
    let (line, _): (&str, &str) = spaces(line).unwrap();  // make me opt!  ??
    let (line, version): (&str, &str) = http_version(line).unwrap();
    match crlf(line) {
      Ok(_) => Ok(RequestLine {method: method, target: target, version: version}),
      Err(_) => Err( FlaskError::BadRequest("Malformed Request Line: no terminating CRLF".to_string()) )
    }
}

fn _read_initial_request_line(reader: &mut BufReader<TcpStream>) -> Result<Builder, FlaskError> {
    let mut request = Request::builder();

    let mut line: String = String::from("");
    match reader.read_line(&mut line) {
        Ok(_) => {
            let req_line: RequestLine = match parse_request_line(line.as_str()) {
                Ok(parsed_line) => parsed_line,
                Err(_) => {
                    let msg = format!("Malformed first line of request");
                    return Err( FlaskError::BadRequest(msg) );
                }
            };
            let http_version = get_http_version(req_line.version)?;

            request = request
                .method(req_line.method)
                .uri(req_line.target)
                .version(http_version);
        },
        Err(_) => {}
    }
    Ok(request)
}


fn _read_http_request(reader: &mut BufReader<TcpStream>) -> Result<Request<Vec<u8>>, FlaskError> {
  let mut request = _read_initial_request_line(reader)?;

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

          if header_line.key.to_lowercase() == CONTENT_LENGTH_HEADER {
              match header_line.value.parse::<usize>() {
                  Ok(val) => content_length_mut = val,
                  Err(_) => {
                      let msg = format!("Invalid Content-Length: {}", header_line.value);
                      return Err( FlaskError::BadRequest(msg) );
                  }
              }
          }
          request = request.header(header_line.key, header_line.value);
      }
      content_length_mut
  };

  let mut body = vec![0; content_length];
  match reader.read_exact(&mut body) {
      Ok(_) => {
          match request.body(body) {
              Ok(req) => Ok(req),
              Err(http_err) => {
                  eprintln!("ERROR reading request body from stream");
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


pub fn read_http_request(stream: TcpStream) -> Result<Request<Vec<u8>>, FlaskError> {
  let mut reader: BufReader<TcpStream> = BufReader::new(stream);
  _read_http_request(&mut reader)
}

//#################################################################################################################
// test cases go below here
//#################################################################################################################
#[cfg(test)]
mod tests {
  // extern crate rand;

  use super::*;

  #[test]
  fn test_parse_request_line__good() {
    let line = "POST  https://panthip.com  HTTP/1.2\r\n";
    match parse_request_line(line) {
      Ok(parsed_line) => {
        assert_eq!(parsed_line.method, "POST");
        assert_eq!(parsed_line.target, "https://panthip.com");
        assert_eq!(parsed_line.version, "1.2");
      },
      Err(e) => {
        assert_eq!(true, false);
      }
    }
  }

  // fn test_parse_request_line__no_crlf() {
  //   let line = "POST  https://panthip.com  HTTP/1.2";
  //   let expected = FlaskError::BadRequest("Malformed Request Line: no terminating CRLF".to_string());

  //   match parse_request_line(line) {
  //     Ok(_) => assert_eq!(true, false), // this test should return an Err
  //     Err(e) => assert_eq!(e, expected)
  //   };
  // }
}
