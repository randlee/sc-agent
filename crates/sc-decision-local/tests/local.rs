#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use sc_decision_local::{Availability, Generic, Laya, LocalDaemon, Mapika, Strands};

#[test]
fn daemons_read_their_own_health() {
    assert_eq!(
        Laya.availability(br#"{"loaded":["m"]}"#, "m").unwrap(),
        Availability::Loaded
    );
    assert_eq!(
        Laya.availability(br#"{"loaded":null}"#, "m").unwrap(),
        Availability::LoadsOnDemand
    );
    assert_eq!(
        Strands.availability(br#"{"model":"x"}"#, "m").unwrap(),
        Availability::Other(vec!["x".into()])
    );
    assert_eq!(
        Mapika
            .availability(br#"{"ok":false,"model":"m"}"#, "m")
            .unwrap(),
        Availability::Loading
    );
    assert_eq!(
        Mapika
            .availability(br#"{"ok":true,"model":"m"}"#, "m")
            .unwrap(),
        Availability::Loaded
    );
    assert_eq!(
        Generic
            .availability(br#"{"models":[{"name":"m"}]}"#, "m")
            .unwrap(),
        Availability::Loaded
    );
}
