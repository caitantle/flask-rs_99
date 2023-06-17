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
    let (line, method): (&str, &str) = match http_method(line) {
      Ok(obj) => obj,
      Err(_) => return Err( FlaskError::BadRequest("Malformed Request Line: missing HTTP method".to_string()) )
    };
    let (line, _): (&str, &str) = match spaces(line) {
      Ok(obj) => obj,
      Err(_) => return Err( FlaskError::BadRequest("Malformed Request Line: missing space before target URL".to_string()) )
    };
    let (line, target): (&str, &str) = match to_space(line) {
      Ok(obj) => obj,
      Err(_) => return Err( FlaskError::BadRequest("Malformed Request Line: error parsing target URL".to_string()) )
    };
    let (line, _): (&str, &str) = match spaces(line) {
      Ok(obj) => obj,
      Err(_) => return Err( FlaskError::BadRequest("Malformed Request Line: missing space after target URL".to_string()) )
    };
    let (line, version): (&str, &str) = match http_version(line) {
      Ok(obj) => obj,
      Err(_) => return Err( FlaskError::BadRequest("Malformed Request Line: bad http version".to_string()) )
    };
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
            let version = get_http_version(req_line.version)?;

            request = request
                .method(req_line.method)
                .uri(req_line.target)
                .version(version);
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
  fn test_parse_request_line_good() {
    let line = "POST  https://panthip.com  HTTP/1.2\r\n";
    let parsed_line = parse_request_line(line).unwrap();
    assert_eq!(parsed_line.method, "POST");
    assert_eq!(parsed_line.target, "https://panthip.com");
    assert_eq!(parsed_line.version, "1.2");
    // match  {
    //   Ok(parsed_line) => {
    //     assert_eq!(parsed_line.method, "POST");
    //     assert_eq!(parsed_line.target, "https://panthip.com");
    //     assert_eq!(parsed_line.version, "1.2");
    //   },
    //   Err(e) => {
    //     assert_eq!(true, false);
    //   }
    // }
  }

  #[test]
  fn test_parse_request_line_bad_http_method() {
    let line = "POS  https://panthip.com  HTTP/1.1\r\n";
    let result = parse_request_line(line);
    let flask_err = result.err().unwrap();
    assert_eq!(flask_err.get_msg(), "Malformed Request Line: missing HTTP method"); 
  }

  #[test]
  fn test_parse_request_line_bad_missing_newline() {
    let line = "POST  https://panthip.com  HTTP/1.1\r";
    let result = parse_request_line(line);
    let flask_err = result.err().unwrap();
    assert_eq!(flask_err.get_msg(), "Malformed Request Line: no terminating CRLF"); 
  }

  #[test]
  fn test_parse_request_line_bad_missing_carriage_return() {
    let line = "POST  https://panthip.com  HTTP/1.1\n";
    let result = parse_request_line(line);
    let flask_err = result.err().unwrap();
    assert_eq!(flask_err.get_msg(), "Malformed Request Line: no terminating CRLF"); 
  }

  #[test]
  fn test_parse_request_line_bad_missing_crlf() {
    let line = "POST  https://panthip.com  HTTP/1.1";
    let result = parse_request_line(line);
    let flask_err = result.err().unwrap();
    assert_eq!(flask_err.get_msg(), "Malformed Request Line: no terminating CRLF"); 
  }
}
