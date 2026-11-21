use sc_agent_types::{BusyReason, Error, ErrorId};

#[test]
fn native_error_fields_keep_their_types_and_messages() {
    let cases = [
        (
            Error::ProviderBusy {
                attempts: 3,
                status: 503,
                retry_after_ms: Some(2001),
                retry_after_seconds: Some(3),
                reason: BusyReason::Sends,
            },
            Some(503),
            Some(3),
            Some(2001),
            "busy",
        ),
        (
            Error::QuotaExhausted {
                status: 429,
                body: "whole remote message".into(),
                attempts: 1,
                retry_after_ms: Some(0),
            },
            Some(429),
            Some(1),
            Some(0),
            "quota",
        ),
        (
            Error::ApiStatus {
                status: 401,
                body: "whole remote message".into(),
                retry_after_ms: Some(u64::MAX),
            },
            Some(401),
            None,
            Some(u64::MAX),
            "auth",
        ),
        (
            Error::Network {
                message: "connection refused".into(),
                attempts: 4,
            },
            None,
            Some(4),
            None,
            "network",
        ),
        (
            Error::Cancelled { sends: 2 },
            None,
            Some(2),
            None,
            "cancelled",
        ),
        (
            Error::ResponseParse {
                message: "bad JSON".into(),
                status: Some(200),
            },
            Some(200),
            None,
            None,
            "response_invalid",
        ),
        (
            Error::Config("invalid config".into()),
            None,
            None,
            None,
            "config",
        ),
    ];
    for (error, status, attempts, retry_after_ms, category) in cases {
        let detail = error.detail();
        assert_eq!(detail.id, error.id());
        assert_eq!(detail.message, error.to_string());
        assert_eq!(
            (detail.status, detail.attempts, detail.retry_after_ms),
            (status, attempts, retry_after_ms)
        );
        assert_eq!(detail.category.as_deref(), Some(category));
        assert_eq!(detail, detail.clone());
    }
}
