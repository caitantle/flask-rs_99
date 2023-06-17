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
    // http_version_num: &'a str,
    status_code: &'a str,
    status_msg: &'a str,
    version: &'a str,
}

use http::{Response, StatusCode};
use http::response::Builder;
use std::io::{
  BufReader,
  prelude::*
};
use std::net::TcpStream;


// "HTTP/1.1 200 OK\r\n";
fn parse_response_line(line: &str) -> Result<ResponseLine, FlaskError> {
  let (line, version): (&str, &str) = match http_version(line) {
    Ok(obj) => obj,
    Err(_) => return Err( FlaskError::BadRequest("Malformed Response Line: bad http version".to_string()) )
  };

  let (line, _): (&str, &str) = match spaces(line) {
    Ok(obj) => obj,
    Err(_) => return Err( FlaskError::BadRequest("Malformed Response Line: no spaces before status code".to_string()) )
  };

  let (line, status_code): (&str, &str) = match number(line) {
    Ok(obj) => obj,
    Err(_) => return Err( FlaskError::BadRequest("Malformed Response Line: no status code".to_string()) )
  };

  let (line, _): (&str, &str) = match spaces(line) {
    Ok(obj) => obj,
    Err(_) => return Err( FlaskError::BadRequest("Malformed Response Line: no spaces after status code".to_string()) )
  };

  let (line, status_msg): (&str, &str) = match take_until_carriage_return(line) {
    Ok(obj) => obj,
    Err(_) => return Err( FlaskError::BadRequest("Malformed Response Line: error parsing status message".to_string()) )
  };

  match crlf(line) {
    Ok(_) => Ok(ResponseLine {status_code: status_code, status_msg: status_msg, version: version}),
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
          let version = get_http_version(resp_line.version)?;

          response = response
              .status(status_code)
              .version(version);
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

pub fn read_http_response(stream: TcpStream) -> Result<Response<Vec<u8>>, FlaskError> {
    let mut reader: BufReader<TcpStream> = BufReader::new(stream);
    _read_http_response(&mut reader)
}


//#################################################################################################################
// test cases go below here
//#################################################################################################################
#[cfg(test)]
mod tests {
    // Test creating an http Response from the mockito response
    extern crate rand;

    use super::*;
    use http::{Version, StatusCode};
    // use mockito::Matcher;
    use std::net::TcpStream;
    use rand::{Rng, thread_rng};
    use rand::distributions::Alphanumeric;
    
    #[test]
    fn test_parse_response_line_ok() {
        let resp_line_str = "HTTP/1.1 200 OK\r\n";
        let parse_result = parse_response_line(resp_line_str);
        assert!(parse_result.is_ok());

        let resp_line = parse_result.unwrap();
        assert_eq!(resp_line.status_code, "200");
        assert_eq!(resp_line.version, "1.1");
    }

    #[test]
    fn test_parse_response_line_server_error() {
        let resp_line_str = "HTTP/1.1 500 Internal Server Error\r\n";
        let parse_result = parse_response_line(resp_line_str);
        assert!(parse_result.is_ok());

        let resp_line = parse_result.unwrap();
        assert_eq!(resp_line.status_code, "500");
        assert_eq!(resp_line.version, "1.1");
    }

    #[test]
    fn test_minimal_get_request() {
        let mut s = mockito::Server::new();

        // define the endpoint for the mock server
        let mock_body = "Hello World!";
        let _mock = s.mock("GET", "/hello")
                        .with_body(mock_body.clone())
                        .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        stream.write_all("GET /hello HTTP/1.1\r\n\r\n".as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, mock_body.len().to_string());

        // verify the response body is correct
        let resp_body: String = String::from_utf8(resp.body().to_vec()).unwrap();
        assert_eq!(resp_body.len(), mock_body.len());
        assert_eq!(resp_body, mock_body);

        _mock.assert();
    }

    #[test]
    fn test_get_request_with_query_string() {
        let mut s = mockito::Server::new();

        let endpoint = "/alpha/beta/gamma?foo=bar&hello=world";
        let mock_body = "I'm a teapot";
        let _mock = s.mock("GET", endpoint)
                        .with_body(mock_body.clone())
                        .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        let payload = format!("GET {} HTTP/1.1\r\n\r\n", endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, mock_body.len().to_string());

        _mock.assert();
    }

        #[test]
    fn test_get_request_with_query_string_not_matched() {
        let mut s = mockito::Server::new();

        let endpoint = "/alpha/beta/gamma?foo=bar";
        let _mock = s.mock("GET", endpoint)
                        .with_status(501)
                        .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        let payload = format!("GET {} HTTP/1.1\r\n\r\n", endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, "0");

        _mock.assert();
    }

    #[test]
    fn test_get_request_with_headers() {
        let mut s = mockito::Server::new();

        let endpoint = "/alpha/beta/gamma?foo=bar&hello=world";
        let mock_body = "it's a small world";
        let _mock = s.mock("GET", endpoint)
                .with_header("puppy", "dog")
                .with_header("GoLd", "fish") // headers names are NOT case sensitive
                .with_body(mock_body.clone())
                .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        let payload = format!("GET {} HTTP/1.1\r\n\r\n", endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, mock_body.len().to_string());

        assert_eq!(resp.headers()["puppy"], "dog");
        assert_eq!(resp.headers()["gold"], "fish");

        _mock.assert();
    }

    #[test]
    fn test_delete_request_with_headers() {
        let mut s = mockito::Server::new();

        let endpoint = "/first/second/third?aaa=bbb&ccc=ddd";
        let mock_body = "this is a delete request";
        let _mock = s.mock("DELETE", endpoint)
                .with_header("fluffy", "bunny")
                .with_header("wet", "dog") // headers names are NOT case sensitive
                .with_body(mock_body.clone())
                .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        let payload = format!("DELETE {} HTTP/1.1\r\n\r\n", endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, mock_body.len().to_string());

        assert_eq!(resp.headers()["fluffy"], "bunny");
        assert_eq!(resp.headers()["wet"], "dog");

        _mock.assert();
    }

    #[test]
    fn test_post_request_with_body() {
        let mut s = mockito::Server::new();

        let mut rng = thread_rng();
        let rand_len = rng.gen_range(10..20);
        let rand_body: String = rng
            .sample_iter(Alphanumeric)
            .take(rand_len.clone())
            .map(char::from)
            .collect();

        let _mock = s.mock("POST", "/foo/bar").with_body(rand_body.clone()).create();

        // Place a request
        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        stream.write_all("POST /foo/bar HTTP/1.1\r\n\r\n".as_bytes()).unwrap();
        stream.flush().unwrap();

        // Read the response
        let resp = read_http_response(stream).unwrap();
        let body: String = String::from_utf8(resp.body().to_vec()).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, rand_len.to_string());
        assert_eq!(body.clone(), rand_body);
        assert_eq!(body.len(), rand_len);

        _mock.assert();
    }

    #[test]
    fn test_post_response_with_large_body() {
        let mut s = mockito::Server::new();

        let mut rng = thread_rng();
        let rand_len = rng.gen_range(1e5..1e6) as usize;
        let rand_body: String = rng
            .sample_iter(Alphanumeric)
            .take(rand_len.clone())
            .map(char::from)
            .collect();

        let _mock = s.mock("POST", "/foo-bar").with_body(rand_body.clone()).create();  // .expect_at_most(1).create();

        // Place a request
        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        stream.write_all("POST /foo-bar HTTP/1.1\r\n\r\n".as_bytes()).unwrap();
        stream.flush().unwrap();

        // Read the response
        let resp_result = read_http_response(stream);
        assert_eq!(resp_result.is_ok(), true);
        let resp = resp_result.unwrap();

        let body: String = String::from_utf8(resp.body().to_vec()).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body.len(), rand_len);
        assert_eq!(body, rand_body);

        _mock.assert();
    }

    #[test]
    fn test_basic_options_request() {
        let mut s = mockito::Server::new();

        let endpoint = "/index.html";
        let status_code = 200;
        let method = "OPTIONS";

        let _mock = s.mock(method, endpoint)
            .with_status(status_code)
            .with_header("Access-Control-Request-Method", "POST")
            .with_header("Access-Control-Request-Headers", "Content-Type,X-My-Header")
            .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        let payload = format!("{} {} HTTP/1.1\r\n\r\n", method, endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status().as_u16(), status_code as u16);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, "0");
        assert_eq!(resp.headers()["Access-Control-Request-Method"], "POST");
        assert_eq!(resp.headers()["Access-Control-Request-Headers"], "Content-Type,X-My-Header");

        _mock.assert();
    }


    #[test]
    fn test_basic_head_request() {
        let mut s = mockito::Server::new();

        let endpoint = "/index.html";
        let status_code = 200;
        let method = "HEAD";

        let _mock = s.mock(method, endpoint)
            .with_status(status_code)
            .create();

        let mut stream = TcpStream::connect(s.host_with_port()).unwrap();
        let payload = format!("{} {} HTTP/1.1\r\n\r\n", method, endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();

        assert_eq!(resp.status().as_u16(), status_code as u16);
        assert_eq!(resp.version(), Version::HTTP_11);

        _mock.assert();
    }

    #[test]
    fn test_bad_http_method() {
      let line = "HTP/1.1 200 OK\r\n";
      let result = parse_response_line(line);
      let flask_err = result.err().unwrap();
      assert_eq!(flask_err.get_msg(), "Malformed Response Line: bad http version");
    }

    #[test]
    fn test_bad_missing_crlf() {
        let line = "HTTP/1.1 200 OK";
      let result = parse_response_line(line);
      let flask_err = result.err().unwrap();
      // without the /r the parser doesn't know when the status message ends
      assert_eq!(flask_err.get_msg(), "Malformed Response Line: error parsing status message");
    }

    #[test]
    fn test_bad_missing_carriage_return() {
      let line = "HTTP/1.1 200 OK\n";
      let result = parse_response_line(line);
      let flask_err = result.err().unwrap();
      assert_eq!(flask_err.get_msg(), "Malformed Response Line: error parsing status message");
    }

    #[test]
    fn test_bad_missing_newline() {
      let line = "HTTP/1.1 200 OK\r";
      let result = parse_response_line(line);
      let flask_err = result.err().unwrap();
      // without the /r the parser doesn't know when the status message ends
      assert_eq!(flask_err.get_msg(), "Malformed Response Line: no terminating CRLF");
    }
}
