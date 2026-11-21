use sc_agent_config::DEFAULT_MAX_SENDS;
use sc_agent_http::ResendPolicy;

#[test]
fn http_and_config_share_the_same_default_send_limit() {
    assert_eq!(
        ResendPolicy::default().max_sends(),
        DEFAULT_MAX_SENDS.get(),
        "REQ-SAG-0601 requires HTTP and configuration defaults to stay equal"
    );
}
