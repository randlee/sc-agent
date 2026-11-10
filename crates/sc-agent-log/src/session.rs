    /// The `sc-observability` event that `event` becomes.
    ///
    /// The conversion is public so the golden test can pin the exact action,
    /// target and fields (REQ-SAG-0611). `fields` is the event's row, cloned
    /// unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`LogRejected`] when the operation spelling is not a valid
    /// action name; the event is rejected, never a panic.
    pub fn log_event(&self, event: &SessionEvent) -> Result<LogEvent, LogRejected> {
        Ok(LogEvent {
            version: SchemaVersion::new(OBSERVATION_ENVELOPE_VERSION)
                .map_err(|_| LogRejected::new("observation envelope version is not valid"))?,
            timestamp: Timestamp::now_utc(),
            level: level_of(event),
            service: self.service.clone(),
            target: TargetCategory::new(TARGET)
                .map_err(|_| LogRejected::new("session log target is not valid"))?,
            action: action_name(event.operation())?,
            message: None,
            identity: ProcessIdentity::default(),
            trace: None,
            // The session correlates its sends; the answer id names one send.
            // Both are ULIDs, valid ids (`sc-observability-types` is not a
            // direct dependency, so the ids are built through serde);
            // `fields` is unchanged.
            request_id: event
                .fields()
                .get("answer_id")
                .and_then(Value::as_str)
                .and_then(|id| envelope_id!(id)),
            correlation_id: envelope_id!(&event.session_id().to_string()),
            outcome: None,
            diagnostic: None,
            state_transition: None,
            fields: event.fields().clone(),
        })
    }
}
