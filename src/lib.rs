use std::{
    borrow::Borrow,
    collections::HashMap,
    error::Error,
    fmt::Display,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
};

type HTTPHandler = fn(HTTPRequest) -> Result<(), Box<dyn Error>>;

pub struct HttpServer {
    port: i32,
    functions: HashMap<(String, Method), HTTPHandler>,
}

impl HttpServer {
    pub fn new(port: i32) -> HttpServer {
        HttpServer {
            port,
            functions: HashMap::new(),
        }
    }

    pub fn get(&mut self, url: String, func: HTTPHandler) {
        self.functions.insert((url, Method::GET), func);
    }

    pub fn post(&mut self, url: String, func: HTTPHandler) {
        self.functions.insert((url, Method::POST), func);
    }

    pub fn add_method(&mut self, method: Method, url: String, func: HTTPHandler) {
        self.functions.insert((url, method), func);
    }

    pub fn listen(&self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("localhost:{}", self.port))?;

        for stream in listener.incoming() {
            log::debug!("Incoming stream");
            if let Some(mut request) = HTTPRequest::new(stream?) {
                let url = request.url.clone();
                let method = request.method;

                let func = self.functions.get(&(url, method));

                match func {
                    Some(f) => f(request)?,
                    None => request.send("404")?,
                }
            }
        }
        Ok(())
    }
}

pub enum Status {
    Ok,
    NotFound,
    SwitchingProtocols,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let res_str = match self {
            Status::Ok => "200 OK",
            Status::NotFound => "404 NOT FOUND",
            Status::SwitchingProtocols => "101 Switching Protocols",
        };
        write!(f, "{}", res_str)
    }
}

pub struct HTTPResponse {
    protocol: String,
    status: Status,
    data: String,
    headers: HashMap<String, String>,
}

impl Display for HTTPResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let headers = self
            .headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<String>>()
            .join("\n");
        write!(
            f,
            "{} {}\r\nContent-Length: {}\r\n{}\r\n{}",
            self.protocol,
            self.status,
            self.data.len(),
            headers,
            self.data
        )
    }
}

impl HTTPResponse {
    pub fn new(status: Status, data: String) -> HTTPResponse {
        HTTPResponse {
            protocol: "HTTP/1.1".to_owned(),
            status,
            data,
            headers: HashMap::new(),
        }
    }

    pub fn add_header(&mut self, key: String, value: String) {
        match self.headers.get(&key) {
            Some(_) => return,
            None => self.headers.insert(key, value),
        };
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Method {
    OPTIONS,
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    TRACE,
    CONNECT,
}

impl From<&str> for Method {
    fn from(value: &str) -> Self {
        match value.to_lowercase().borrow() {
            "options" => Self::OPTIONS,
            "get" => Self::GET,
            "head" => Self::HEAD,
            "post" => Self::POST,
            "put" => Self::PUT,
            "delete" => Self::DELETE,
            "trace" => Self::TRACE,
            "connect" => Self::CONNECT,
            _ => panic!("Invalid conversion to Method from String: {}", value),
        }
    }
}

impl Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let res_str = match self {
            Method::OPTIONS => "OPTIONS",
            Method::GET => "GET",
            Method::HEAD => "HEAD",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::DELETE => "DELETE",
            Method::TRACE => "TRACE",
            Method::CONNECT => "CONNECT",
        };

        write!(f, "{}", res_str)
    }
}

//GET / HTTP/1.1
//Host: localhost:5000
//User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/110.0
//Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8
//Accept-Language: en-US,en;q=0.5
//Accept-Encoding: gzip, deflate, br
//Connection: keep-alive
//Upgrade-Insecure-Requests: 1
//Sec-Fetch-Dest: document
//Sec-Fetch-Mode: navigate
//Sec-Fetch-Site: none
//
//Sec-Fetch-User: ?1

pub struct HTTPRequest {
    method: String,
    target: String,
    http_version: String,
    headers: HashMap<String, String>,
    body: String,
    stream: TcpStream,
}

impl HTTPRequest {
    pub fn new(stream: TcpStream) -> Option<HTTPRequest> {
        let mut bufreader = BufReader::new(stream);
        let mut first_line = String::new();
        bufreader.by_ref().read_line(&mut first_line).unwrap();
        let mut first_line = first_line.split(' ');
        let method = first_line.next().unwrap();
        let target = first_line.next().unwrap();
        let http_version = first_line.next().unwrap().trim_end();
        let mut headers = HashMap::new();

        let mut line = String::new();
        while bufreader.by_ref().read_line(&mut line).unwrap() != 0 {
            if line.is_empty() || !line.contains(": ") {
                break;
            }
            let mut line_split = line.split(": ");
            let key = line_split.next().unwrap();
            let value = line_split.next().unwrap().trim_end();
            headers.insert(
                key.to_string().to_lowercase(),
                value.to_string().to_lowercase(),
            );
            line.clear();
        }

        let mut body = String::new();
        if headers.contains_key("content-length") {
            let size_string = headers.get("content-length").unwrap();
            println!("size_string: '{}'", size_string);
            let size = headers
                .get("content-length")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let mut buf = vec![0; size];
            bufreader.read_exact(&mut buf).unwrap();
            body = String::from_utf8(buf).unwrap();
        }

        Some(HTTPRequest {
            method: method.to_string(),
            target: target.to_string(),
            http_version: http_version.to_string(),
            headers,
            body,
            stream,
        })
    }

    pub fn send(&mut self, text: &str) -> Result<(), Box<dyn Error>> {
        let response = HTTPResponse::new(Status::Ok, text.to_string());
        self.stream.write_all(response.to_string().as_bytes())?;
        Ok(())
    }

    pub fn send_json(&mut self, text: &str) -> Result<(), Box<dyn Error>> {
        let mut response = HTTPResponse::new(Status::Ok, text.to_string());
        response.add_header("Content-Type".to_string(), "application/json".to_string());
        self.stream.write_all(response.to_string().as_bytes())?;
        Ok(())
    }

    pub fn get_headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

