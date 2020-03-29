use super::{
    http,
    http_version,
    read_header,
    http_method,
    to_space
};

use crate::combinators::{
    crlf,
    slash,
    spaces
};

use http::{Request, Version};
use http::request::Builder;
use std::io::{
    self,
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

fn _read_initial_request_line(reader: &mut BufReader<TcpStream>) -> Result<Builder, http::Error> {
    let mut request = Request::builder();

    let mut line: String = String::from("");
    match reader.read_line(&mut line) {
        Ok(_) => {
            let (_, req_line) = parse_request_line(line.as_bytes()).unwrap();

            request = request
                .method(req_line.method)
                .uri(req_line.target);

            request = match req_line.version {
                "0.9" => request.version( Version::HTTP_09 ),
                "1.0" => request.version( Version::HTTP_10 ),
                "1.1" => request.version( Version::HTTP_11 ),
                "2.0" => request.version( Version::HTTP_2 ),
                "3.0" => request.version( Version::HTTP_3 ),
                _ => { request }  // I don't know the http version so skip it
            };
        },
        Err(_) => {}
    }
    Ok(request)
}

fn _read_http_request(reader: &mut BufReader<TcpStream>) -> Result<Request<Vec<u8>>, http::Error> {
    let mut request = _read_initial_request_line(reader)?;

    let mut content_length = 0;

    loop {
        let mut line: String = String::from("");
        let num_bytes_result: Result<usize, io::Error> = reader.read_line(&mut line);

        let num_bytes = num_bytes_result.unwrap();

        if num_bytes == 2 && line.as_str() == "\r\n" {
            break;
        }

        let (_, header_line) = read_header(line.as_bytes()).unwrap();

        if header_line.key.to_lowercase() == "content-length" {
            content_length = header_line.value.parse::<usize>().unwrap();
        }
        request = request.header(header_line.key, header_line.value);
    }

    let mut body = vec![0; content_length];
    if reader.read_exact(&mut body).is_err() {
        eprintln!("ERROR reading request body from stream");
    }

    request.body(body)
}

pub fn read_http_request(stream: TcpStream) -> Result<Request<Vec<u8>>, http::Error> {
    let mut reader: BufReader<TcpStream> = BufReader::new(stream);

    _read_http_request(&mut reader)
}
