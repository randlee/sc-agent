//! A5-19: the feature-`testing` doubles behave as documented. No sleep.

/// Clippy treats only `#[test]` functions and `#[cfg(test)]` items as test
/// code (where clippy.toml allows unwrap, expect, panic and indexing); the
/// helpers here are test code, so the whole file is one `#[cfg(test)]` module.
#[cfg(test)]
mod cases {

    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    use sc_agent_http::testing::{
        BlockingTransport, ManualClock, RecordingTransport, ScriptedTransport,
    };
    use sc_agent_http::{
        Endpoint, HttpError, HttpRequest, HttpResponse, Method, NetworkKind, Transport,
    };

    fn request(path: &str) -> HttpRequest {
        HttpRequest::new(
            Method::Get,
            Endpoint::parse(&format!("https://doubles.test{path}")).expect("https"),
            Vec::new(),
        )
    }

    fn status(code: u16) -> HttpResponse {
        HttpResponse::new(code, Vec::new(), Vec::new())
    }

    #[test]
    fn scripted_and_blocking_doubles() {
        // Script order, shared record, exhaustion error.
        let clock = ManualClock::new(Instant::now());
        let start = clock.now();
        let scripted = ScriptedTransport::new(vec![
            Ok(status(201)),
            Err(HttpError::network(NetworkKind::Tls, 1, "tls".to_owned())),
        ])
        .with_send_duration(clock.clone(), Duration::from_secs(3));
        let twin = scripted.clone();
        assert_eq!(scripted.send(&request("/a")).map(|r| r.status()), Ok(201));
        assert!(matches!(
            twin.send(&request("/b")),
            Err(HttpError::Network {
                kind: NetworkKind::Tls,
                ..
            })
        ));
        match scripted.send(&request("/c")) {
            Err(HttpError::Network {
                kind: NetworkKind::Connect,
                message,
                ..
            }) => assert_eq!(message, "script exhausted"),
            other => panic!("{other:?}"),
        }
        let paths: Vec<String> = twin
            .requests()
            .iter()
            .map(|sent| sent.endpoint().as_str().to_owned())
            .collect();
        assert_eq!(
            paths,
            [
                "https://doubles.test/a",
                "https://doubles.test/b",
                "https://doubles.test/c"
            ]
        );
        assert_eq!(scripted.sends(), 3);
        assert_eq!(clock.now() - start, Duration::from_secs(9));

        // Recording in front of another transport.
        let recording = RecordingTransport::new(ScriptedTransport::responses(vec![status(204)]));
        assert_eq!(recording.send(&request("/r")).map(|r| r.status()), Ok(204));
        assert_eq!(recording.requests().len(), 1);

        // BlockingTransport returns only after release().
        let blocking = BlockingTransport::new(Ok(status(200)));
        let (done_tx, done_rx) = mpsc::channel();
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let sender = blocking.clone();
                let done = done_tx.clone();
                thread::spawn(move || {
                    let result = sender.send(&request("/blocked")).map(|r| r.status());
                    done.send(()).expect("report");
                    result
                })
            })
            .collect();
        blocking.wait_started(2);
        assert_eq!(blocking.sends(), 2);
        // Both are inside send() and neither has returned.
        assert!(done_rx.try_recv().is_err());
        blocking.release();
        for worker in workers {
            assert_eq!(worker.join().expect("worker"), Ok(200));
        }
        assert_eq!(done_rx.iter().take(2).count(), 2);
        // After release, later sends return at once.
        assert_eq!(
            blocking.send(&request("/late")).map(|r| r.status()),
            Ok(200)
        );
        assert_eq!(blocking.sends(), 3);
    }
}
