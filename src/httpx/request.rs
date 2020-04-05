use super::{
    CONTENT_LENGTH_HEADER,
    FlaskError,
    get_http_version,
    http,
    http_version,
    http_method,
    read_buffered_line,
    read_header,
    to_space
};

use crate::combinators::{
    crlf,
    slash,
    spaces
};

use http::Request;
use http::request::Builder;
use std::io::{
    BufReader,
    prelude::*
};
use std::net::TcpStream;
use std::str;

#[derive(PartialEq, Debug)]
struct RequestLine<'a> {
    method: &'a str,
    target: &'a str,
    version: &'a str,
}


named!( parse_request_line <RequestLine>,
    do_parse!(
        method: http_method >> opt!(spaces) >> target: to_space >> opt!(spaces) >>
        http >> slash >> version: http_version >> crlf >>
        (RequestLine {method: method, target: target , version: version})
    )
);

fn _read_initial_request_line(reader: &mut BufReader<TcpStream>) -> Result<Builder, FlaskError> {
    let mut request = Request::builder();

    let mut line: String = String::from("");
    match reader.read_line(&mut line) {
        Ok(_) => {
            let (_, req_line) = parse_request_line(line.as_bytes()).unwrap();
            let http_version = get_http_version(req_line.version).unwrap();

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

            let (_, header_line) = read_header(line.as_bytes()).unwrap();

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
