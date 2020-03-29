use super::{
    CONTENT_LENGTH_HEADER,
    FlaskError,
    get_http_version,
    http,
    http_version,
    read_buffered_line,
    read_header,
    token
};

use crate::combinators::{
    crlf,
    number,
    slash,
    spaces
};

use http::{Response, StatusCode};
use http::response::Builder;
use std::io::{
    BufReader,
    prelude::*
};
use std::net::TcpStream;


#[derive(PartialEq, Debug)]
struct ResponseLine<'a> {
    status_code: &'a str,
    version: &'a str,
}

named!( parse_response_line <ResponseLine>,
    do_parse!(
        http >> slash >> version: http_version >> opt!(spaces) >>
        status_code: number >> opt!(spaces) >> token >> crlf >>
        (ResponseLine {status_code: status_code, version: version})
    )
);

fn _read_initial_request_line(reader: &mut BufReader<TcpStream>) -> Result<Builder, FlaskError> {
    let mut response = Response::builder();

    let mut line: String = String::from("");
    match reader.read_line(&mut line) {
        Ok(_) => {
            let (_, resp_line) = parse_response_line(line.as_bytes()).unwrap();

            let status_code_bytes = resp_line.status_code.as_bytes();
            let status_code = StatusCode::from_bytes(status_code_bytes).unwrap();
            let http_version = get_http_version(resp_line.version).unwrap();

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

            let (_, header_line) = read_header(line.as_bytes()).unwrap();
            // println!("{:?}", line.as_bytes());

            if header_line.key.to_lowercase() == CONTENT_LENGTH_HEADER {
                content_length_mut = header_line.value.parse::<usize>().unwrap();
            }
            // println!("Key => {}", elem.key);
            response = response.header(header_line.key, header_line.value);
        }
        content_length_mut
    };

    let mut body = vec![0; content_length];
    if reader.read_exact(&mut body).is_err() {
        eprintln!("ERROR reading response body from stream");
    }

    match response.body(body) {
        Ok(resp) => Ok(resp),
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
    use mockito::{mock, server_address};
    use std::net::TcpStream;
    use rand::{Rng, thread_rng};
    use rand::distributions::Alphanumeric;
    
    #[test]
    fn test_minimal_get_request() {
        let mock_body = "Hello World!";
        let _mock = mock("GET", "/hello").with_body(mock_body.clone()).create();

        let mut stream = TcpStream::connect(server_address()).unwrap();
        stream.write_all("GET /hello HTTP/1.1\r\n\r\n".as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, mock_body.len().to_string());
        // assert!(resp.body().is_empty());

        // verify the response body is correct
        let resp_body: String = String::from_utf8(resp.body().to_vec()).unwrap();
        assert_eq!(resp_body.len(), mock_body.len());
        assert_eq!(resp_body, mock_body);

        _mock.assert();
    }

    #[test]
    fn test_get_request_with_query_string() {
        let endpoint = "/alpha/beta/gamma?foo=bar&hello=world";
        let mock_body = "I'm a teapot";
        let _mock = mock("GET", endpoint).with_body(mock_body.clone()).create();

        let mut stream = TcpStream::connect(server_address()).unwrap();
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
    fn test_get_request_with_headers() {
        let endpoint = "/alpha/beta/gamma?foo=bar&hello=world";
        let mock_body = "it's a small world";
        let _mock = mock("GET", endpoint)
                .with_header("puppy", "dog")
                .with_header("GoLd", "fish") // headers names are NOT case sensitive
                .with_body(mock_body.clone())
                .create();

        let mut stream = TcpStream::connect(server_address()).unwrap();
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
    fn test_post_request_with_body() {
        let mut rng = thread_rng();
        let rand_len = rng.gen_range(10, 20);
        let rand_body: String = rng
            .sample_iter(Alphanumeric)
            .take(rand_len.clone())
            .collect();

        let _mock = mock("POST", "/foo/bar").with_body(rand_body.clone()).create();

        // Place a request
        let mut stream = TcpStream::connect(server_address()).unwrap();
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
        let mut rng = thread_rng();
        let rand_len: usize = rng.gen_range(1e5 as usize, 1e6 as usize);
        let rand_body: String = rng
            .sample_iter(Alphanumeric)
            .take(rand_len.clone())
            .collect();

        let _mock = mock("POST", "/foo-bar").with_body(rand_body.clone()).create();  // .expect_at_most(1).create();

        // Place a request
        let mut stream = TcpStream::connect(server_address()).unwrap();
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
        let endpoint = "/";
        let status_code = 100;

        let _mock = mock("OPTIONS", endpoint)
            .with_status(status_code)
            .create();

        let mut stream = TcpStream::connect(server_address()).unwrap();
        let payload = format!("OPTIONS {} HTTP/1.1\r\n\r\n", endpoint);
        stream.write_all(payload.as_bytes()).unwrap();

        let resp = read_http_response(stream).unwrap();
        let content_length = resp.headers()[http::header::CONTENT_LENGTH].to_str().unwrap();

        assert_eq!(resp.status(), StatusCode::CONTINUE);  // value is 100
        assert_eq!(resp.status().as_u16(), status_code as u16);
        assert_eq!(resp.version(), Version::HTTP_11);
        assert_eq!(content_length, "0");

        _mock.assert();
    }
}
