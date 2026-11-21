//! Controlled inputs exercise the same validator used by Python and the CLI.
use std::time::Duration;

#[test]
fn limits_are_per_call_and_keep_fractional_seconds() {
    assert_eq!(
        decidr::validate_limits(Some(10.125), Some(2)).unwrap(),
        (Some(Duration::from_millis(10125)), Some(2))
    );
    assert_eq!(decidr::validate_limits(None, None).unwrap(), (None, None));
    assert_eq!(
        decidr::validate_limits(Some(3600.0), Some(64)).unwrap(),
        (Some(Duration::from_secs(3600)), Some(64))
    );
}

#[test]
fn invalid_limits_keep_value_id_and_range() {
    for value in [9.99, 3600.01, f64::NAN, f64::INFINITY] {
        let error = decidr::validate_limits(Some(value), None).unwrap_err();
        assert_eq!(error.id(), "decidr.invalid_request_timeout");
        assert!(error.to_string().contains("10..=3600"));
        match error {
            sc_agent_types::Error::InvalidRequestTimeout(actual) => {
                assert!(actual == value || actual.is_nan() && value.is_nan())
            }
            _ => panic!("wrong error variant"),
        }
    }
    for value in [-1, 0, 65] {
        let error = decidr::validate_limits(None, Some(value)).unwrap_err();
        assert_eq!(error.id(), "decidr.invalid_workers");
        assert!(error.to_string().contains(&value.to_string()));
        assert!(error.to_string().contains("1..=64"));
        assert!(!error.retryable());
    }
}
