use sc_agent::{Options, Settings};
use sc_observability::{LoggerConfig, ServiceName, v2::Logger};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::JoinHandle,
    time::Duration,
};
pub type Test = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = std::env::temp_dir().join(format!("sc-agent-test-{}", ulid::Ulid::new()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn logger(root: &Temp) -> Result<Logger, Box<dyn std::error::Error + Send + Sync>> {
    let mut config = LoggerConfig::default_for(ServiceName::new("rust-test")?, root.0.clone());
    config.enable_console_sink = false;
    Ok(Logger::new(config)?)
}
pub fn options(root: &Temp, url: &str) -> Options {
    Options {
        environment: vec![
            ("TYPESAFE_API_KEY".into(), "test-secret".into()),
            ("ANTHROPIC_API_KEY".into(), "test-secret".into()),
            ("OPENAI_API_KEY".into(), "test-secret".into()),
        ],
        settings: Settings {
            project: Some("phase-b".into()),
            workers: 1,
            laya: None,
        },
        model: "jev".into(),
        provider: None,
        agent_role: "rust-developer".into(),
        timeout: Duration::from_secs(5),
        base_url: Some(url.into()),
        root: root.0.clone(),
        log_root: root.0.clone(),
        repo: "github.com/randlee/sc-agent".into(),
    }
}
pub fn request() -> Value {
    json!({"state":"evidence", "questions":{"q":{"type":"noul"}}})
}
pub fn answer() -> Value {
    json!({"model":"jev-1.14.0", "answers":{"q":{"type":"noul","noul":0.123456789}}})
}
pub enum Reply {
    Http(u16, Vec<(&'static str, &'static str)>, String),
    HttpIfComplete(u16, Vec<(&'static str, &'static str)>, String),
    Disconnect,
    Never,
}
pub fn http(status: u16, body: Value) -> Reply {
    Reply::Http(status, vec![], body.to_string())
}
#[derive(Debug)]
pub struct Request {
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}
type CapturedRequests = Result<Vec<Request>, Box<dyn std::error::Error + Send + Sync>>;
fn read_request(
    stream: &mut TcpStream,
) -> Result<Request, Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let path = line
        .split_whitespace()
        .nth(1)
        .ok_or("missing HTTP path")?
        .to_owned();
    let mut headers = vec![];
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        if line == "\r\n" {
            break;
        }
        let (key, value) = line.split_once(':').ok_or("missing header separator")?;
        headers.push((key.to_ascii_lowercase(), value.trim().to_owned()));
    }
    let length: usize = headers
        .iter()
        .find(|(key, _)| key == "content-length")
        .ok_or("missing content length")?
        .1
        .parse()?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    drop(reader);
    Ok(Request {
        path,
        headers,
        body: serde_json::from_slice(&body)?,
    })
}
pub struct Stub {
    pub url: String,
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<CapturedRequests>>,
    observed: Receiver<()>,
    notify: Sender<()>,
}
impl Stub {
    pub fn new(replies: Vec<Reply>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}/v1", listener.local_addr()?);
        let owned_listener = listener.try_clone()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let (notify, observed) = mpsc::channel();
        let server_notify = notify.clone();
        let worker = std::thread::spawn(move || {
            let mut seen = vec![];
            let mut unanswered = vec![];
            for reply in replies {
                let (mut stream, _) = listener.accept()?;
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                // Deadlines are hang guards only; no test waits for or asserts on elapsed time.
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                let request = match read_request(&mut stream) {
                    Ok(request) => request,
                    Err(_) if matches!(&reply, Reply::Never | Reply::HttpIfComplete(..)) => {
                        // A timeout during connect/write is still an attempted send.
                        seen.push(Request {
                            path: String::new(),
                            headers: vec![],
                            body: Value::Null,
                        });
                        let _ = server_notify.send(());
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                seen.push(request);
                let _ = server_notify.send(());
                match reply {
                    Reply::Disconnect => stream.shutdown(std::net::Shutdown::Both)?,
                    Reply::Never => unanswered.push(stream),
                    Reply::Http(status, headers, body)
                    | Reply::HttpIfComplete(status, headers, body) => {
                        write!(
                            stream,
                            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
                            body.len()
                        )?;
                        for (key, value) in headers {
                            write!(stream, "{key}: {value}\r\n")?;
                        }
                        write!(stream, "\r\n{body}")?;
                    }
                }
            }
            drop(unanswered);
            Ok(seen)
        });
        Ok(Self {
            url,
            listener: owned_listener,
            stop,
            worker: Some(worker),
            observed,
            notify,
        })
    }
    pub fn wait_for_request(&self) -> Result<(), mpsc::RecvError> {
        self.observed.recv()
    }
    pub fn request_signal(&self) -> Sender<()> {
        self.notify.clone()
    }
    pub fn finish(mut self) -> Result<Vec<Request>, Box<dyn std::error::Error + Send + Sync>> {
        self.stop();
        self.worker
            .take()
            .ok_or("stub already finished")?
            .join()
            .map_err(|_| "stub thread panicked")?
    }
    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        // Keep a bound listener alive until shutdown finishes; the wake-up can never reach a recycled port.
        if let Ok(address) = self.listener.local_addr() {
            let _ = TcpStream::connect(address);
        }
    }
}
impl Drop for Stub {
    fn drop(&mut self) {
        if self.worker.is_some() {
            self.stop();
        }
    }
}
pub fn keyring(_: &str) -> Result<String, String> {
    Err("test must not access keyring".into())
}
pub fn no_wait(_: Duration) {}
pub fn rows(root: &Temp) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
