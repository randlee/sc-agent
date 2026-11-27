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
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver},
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
pub fn logger(root: &Temp) -> Result<Arc<Logger>, Box<dyn std::error::Error + Send + Sync>> {
    let mut config = LoggerConfig::default_for(ServiceName::new("rust-test")?, root.0.clone());
    config.enable_console_sink = false;
    Ok(Arc::new(Logger::new(config)?))
}
pub fn options(root: &Temp, url: &str) -> Options {
    Options {
        tool_version: "test".into(),
        environment: vec![
            ("TYPESAFE_API_KEY".into(), "test-secret".into()),
            ("ANTHROPIC_API_KEY".into(), "test-secret".into()),
            ("OPENAI_API_KEY".into(), "test-secret".into()),
        ],
        settings: Settings {
            tool: "decidr".into(),
            project: Some("phase-b".into()),
            workers: 1,
            cache: true,
            models: Default::default(),
            local: Default::default(),
            max_sends: 5,
            decidr: Default::default(),
        },
        model: "jev".into(),
        agent_role: "rust-developer".into(),
        timeout: Duration::from_secs(5),
        base_url: Some(url.into()),
        root: root.0.clone(),
        log_root: root.0.clone(),
        repo: "github.com/randlee/sc-agent".into(),
    }
}
pub fn single_reply_options(root: &Temp, url: &str) -> Options {
    let mut options = options(root, url);
    // Failure-only hang guard, not a response-speed assertion; never resend to an exhausted stub.
    options.timeout = Duration::from_secs(600);
    options.settings.max_sends = 1;
    options
}
pub fn request() -> Value {
    json!({"state":"evidence", "questions":{"q":{"type":"noul"}}})
}
pub fn answer() -> Value {
    json!({"model":"jev-1.14.0", "answers":{"q":{"type":"noul","noul":0.123456789}}})
}
pub enum Reply {
    Http(u16, Vec<(&'static str, &'static str)>, String),
    Raw(u16, Vec<(&'static str, &'static str)>, Vec<u8>),
    Disconnect,
    Truncated,
    Redirect(String),
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
    let length = headers
        .iter()
        .find(|(key, _)| key == "content-length")
        .map(|(_, value)| value.parse::<usize>())
        .transpose()?
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    drop(reader);
    Ok(Request {
        path,
        headers,
        body: if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body)?
        },
    })
}
pub struct Stub {
    pub url: String,
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    outcome: Receiver<CapturedRequests>,
    remaining: Arc<AtomicUsize>,
}
impl Stub {
    pub fn new(replies: Vec<Reply>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}/v1", listener.local_addr()?);
        let owned_listener = listener.try_clone()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let remaining = Arc::new(AtomicUsize::new(replies.len()));
        let unserved = remaining.clone();
        let (finished, outcome) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut seen = vec![];
            let mut stage = "accept";
            let mut index = 0;
            let result: CapturedRequests = (|| {
                for (i, reply) in replies.into_iter().enumerate() {
                    index = i + 1;
                    stage = "accept";
                    let (mut stream, _) = listener.accept()?;
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    // Deadlines are hang guards only; no test waits for or asserts on elapsed time.
                    stage = "set read timeout";
                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    stage = "set write timeout";
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                    stage = "read request";
                    let request = read_request(&mut stream)?;
                    seen.push(request);
                    stage = "write response";
                    match reply {
                        Reply::Redirect(location) => {
                            write!(
                                stream,
                                "HTTP/1.1 302 Test\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            )?;
                        }
                        Reply::Disconnect => stream.shutdown(std::net::Shutdown::Both)?,
                        Reply::Truncated => {
                            write!(
                                stream,
                                "HTTP/1.1 200 Test\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{{"
                            )?;
                        }
                        Reply::Raw(status, headers, body) => {
                            write!(stream, "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n", body.len())?;
                            for (key, value) in headers {
                                write!(stream, "{key}: {value}\r\n")?;
                            }
                            write!(stream, "\r\n")?;
                            stream.write_all(&body)?;
                        }
                        Reply::Http(status, headers, body) => {
                            write!(
                                stream,
                                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n",
                                body.len()
                            )?;
                            for (key, value) in headers {
                                write!(stream, "{key}: {value}\r\n")?;
                            }
                            write!(stream, "\r\n")?;
                            for chunk in body.as_bytes().chunks(64 * 1024) {
                                stream.write_all(chunk)?;
                            }
                        }
                    }
                    unserved.fetch_sub(1, Ordering::SeqCst);
                }
                Ok(())
            })()
            .map(|()| std::mem::take(&mut seen))
            .map_err(|error: Box<dyn std::error::Error + Send + Sync>| {
                format!("stub reply {index} at {stage}: {error}; captured requests: {seen:?}").into()
            });
            let _ = finished.send(result);
        });
        Ok(Self {
            url,
            listener: owned_listener,
            stop,
            worker: Some(worker),
            outcome,
            remaining,
        })
    }
    pub fn finish(mut self) -> Result<Vec<Request>, Box<dyn std::error::Error + Send + Sync>> {
        let worker = self.worker.take().ok_or("stub already finished")?;
        self.stop()?;
        let result = self
            .outcome
            .recv_timeout(Duration::from_secs(10))
            .map_err(|e| {
                format!(
                    "stub worker did not finish; {} unserved replies: {e}",
                    self.remaining.load(Ordering::SeqCst)
                )
            })?;
        worker.join().map_err(|_| "stub thread panicked")?;
        result
    }
    fn stop(&self) -> std::io::Result<()> {
        self.stop.store(true, Ordering::SeqCst);
        // Keep a bound listener alive until shutdown finishes; the wake-up can never reach a recycled port.
        TcpStream::connect_timeout(&self.listener.local_addr()?, Duration::from_secs(5))?;
        Ok(())
    }
}
impl Drop for Stub {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let result = self
                .stop()
                .map_err(|e| format!("stub wake-up failed: {e}"))
                .and_then(|()| {
                    self.outcome
                        .recv_timeout(Duration::from_secs(10))
                        .map_err(|e| {
                            format!(
                                "stub worker did not finish; {} unserved replies: {e}",
                                self.remaining.load(Ordering::SeqCst)
                            )
                        })?
                        .map_err(|e| e.to_string())
                })
                .map(|_| ());
            if let Err(error) = result {
                assert!(std::thread::panicking(), "{error}");
                eprintln!("{error}");
            }
        }
    }
}
pub fn keyring(_: &str) -> Result<String, sc_agent::KeyringError> {
    Err(sc_agent::KeyringError::Read(
        "test must not access keyring".into(),
    ))
}
pub fn no_wait(_: Duration) -> Result<(), sc_agent::Error> {
    Ok(())
}
pub fn rows(root: &Temp) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let path = root
        .0
        .join("sessions/github.com/randlee/sc-agent/rust-developer/phase-b/sessions.jsonl");
    fs::read_to_string(path)?
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
                .and_then(|event| {
                    event
                        .get("fields")
                        .cloned()
                        .ok_or_else(|| "missing event fields".into())
                })
        })
        .collect()
}

pub fn run(options: Options, logger: &Arc<Logger>) -> Result<sc_agent::Run, sc_agent::Error> {
    let mut run = sc_agent::Run::from_options(
        options,
        Some(logger.clone()),
        Arc::new(sc_agent::Cancel::default()),
    )?;
    let time = Arc::new(std::sync::Mutex::new(Duration::ZERO));
    let now = time.clone();
    run.now = Box::new(move || {
        *now.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    });
    run.wait = Box::new(move |delay| {
        let mut now = time
            .lock()
            .map_err(|e| sc_agent::error("http.network", e))?;
        *now = now.saturating_add(delay);
        Ok(())
    });
    run.keyring = Box::new(keyring);
    Ok(run)
}

// A pinned provider/model is fixture data for Route wire tests; product selection uses Run.
pub fn pinned_model(
    provider: &str,
    name: &str,
) -> Result<(&'static sc_agent::Provider, String), sc_agent::Error> {
    let provider = sc_agent::PROVIDERS
        .iter()
        .find(|p| p.name == provider)
        .ok_or_else(|| sc_agent::error("types.unknown_provider", provider))?;
    let id = provider
        .models
        .iter()
        .find(|(alias, id, _)| *alias == name || *id == name)
        .map(|(_, id, _)| *id)
        .unwrap_or(name);
    Ok((provider, id.into()))
}
