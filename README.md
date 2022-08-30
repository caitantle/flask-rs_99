[![Build Status](https://travis-ci.org/bpmason1/flask-rs.svg?branch=master)](https://travis-ci.org/bpmason1/flask-rs)
[![minimum rustc 1.31](https://img.shields.io/badge/rustc-1.31+-yellow.svg)

# flask

Flask is a tool for storing the contents of the TcpStream and creating an http Request/Response

## using flask to reverse proxy incoming HTTP requests from a TCPStream to address SocketAddr
```
use flask::httpx::read_http_request;

fn proxy_tcp_stream(stream: TcpStream, proxy_addr: SocketAddr) {
    let _proxy_add_str = format!("{}", proxy_addr);
    let proxy_addr_hdr = HeaderValue::from_str(&_proxy_add_str).unwrap();

    let mut req = read_http_request(stream.try_clone().unwrap()).unwrap();
    
    // In a real system you should implement remove_hop_by_hop_headers.  It's commented out here for simplicity.
    // *req.headers_mut() = remove_hop_by_hop_headers(req.headers());

    let req_headers = req.headers_mut();
    req_headers.remove(http::header::HOST);
    req_headers.insert(http::header::HOST, proxy_addr_hdr);
    handle_request(stream, req);
}
```
