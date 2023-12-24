use anyhow::Result;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};

#[derive(Debug)]
struct Request {
    method: Method,
    path: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}

struct Response {
    status: Status,
    headers: HashMap<String, String>,
    body: String,
}

impl Response {
    fn new(status: Status) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body: String::new(),
        }
    }

    fn with_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_owned(), value.to_owned());
        self
    }

    fn with_body(mut self, body: &str) -> Self {
        self.body = body.to_owned();
        self
    }
}

#[derive(Debug)]
enum Method {
    Get,
    Post,
    Put,
    Delete,
}

enum Status {
    Http200,
    Http400,
    Http404,
}

fn parse_to_request(stream: &mut BufReader<&TcpStream>) -> Result<Request> {
    let mut line = String::new();
    stream.read_line(&mut line)?;

    let line = line.trim_end();

    let parts: Vec<_> = line.splitn(3, ' ').collect();
    if parts.len() != 3 {
        return Err(anyhow::anyhow!("invalid request"));
    }

    let method = match parts[0] {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "PUT" => Method::Put,
        "DELETE" => Method::Delete,
        _ => return Err(anyhow::anyhow!("invalid method")), // return 405
    };

    let path = parts[1].to_owned();

    match parts[2] {
        "HTTP/1.1" => {}
        _ => return Err(anyhow::anyhow!("invalid version")),
    };

    let mut headers = HashMap::new();

    loop {
        let mut line = String::new();
        stream.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let parts: Vec<_> = line.splitn(2, ": ").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("invalid header"));
        }
        headers.insert(parts[0].to_owned(), parts[1].to_owned());
    }

    let content_length = headers
        .get("Content-Length")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);

    if content_length > 1024 {
        return Err(anyhow::anyhow!("content too long"));
    }

    let body = if content_length > 0 {
        let mut buf = [0u8; 1024];
        stream.read(&mut buf)?;
        Some(
            buf[..content_length]
                .iter()
                .filter(|&&c| c != 0)
                .map(|&c| c as char)
                .collect(),
        )
    } else {
        None
    };

    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn write_response(response: Response, stream: &mut BufWriter<&TcpStream>) -> Result<()> {
    let status = match response.status {
        Status::Http200 => "200 OK",
        Status::Http400 => "400 Bad Request",
        Status::Http404 => "404 Not Found",
    };

    stream.write_all(format!("HTTP/1.1 {}\r\n", status).as_bytes())?;

    for (key, value) in response.headers {
        stream.write_all(format!("{}: {}\r\n", key, value).as_bytes())?;
    }

    stream.write_all(b"\r\n")?;
    stream.write_all(response.body.as_bytes())?;

    Ok(())
}

fn handle_request(stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let request = parse_to_request(&mut reader);

    println!("{:?}", request);

    let response = match request {
        Ok(request) => match request.path.as_str() {
            "/" => Response::new(Status::Http200).with_body("Hello World\n"),
            _ => Response::new(Status::Http404),
        },
        Err(_) => Response::new(Status::Http400),
    };
    let mut writer = BufWriter::new(&stream);
    write_response(response, &mut writer).unwrap();
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    println!("listening started, ready to accept");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("accepted new connection");
                handle_request(stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
