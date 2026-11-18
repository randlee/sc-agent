"""Controlled tests for the shared result adapter; no provider or keychain access."""
import json
import sys

import pytest
import sc_agent


COMMON = dict(model="jev", project="consumer", agent_role="judge", abort_on=frozenset({"auth"}))
CATEGORY_CASES = [
    ("auth", "AUTH.CREDENTIALS", False), ("quota", "DECIDR.RATE_LIMITED", False),
    ("busy", "DECIDR.OVERLOADED", True), ("network", "DECIDR.NETWORK", True),
    ("response_invalid", "DECIDR.RESPONSE_INVALID", True), ("validation", "DECIDR.VALIDATION", False),
    ("cancelled", "CANCELED", False), ("config", "DECIDR.CONFIG", False),
    ("log", "DECIDR.LOG", False), ("other", "DECIDR.ERROR", False),
]


def error_fields(error):
    return {name: getattr(error, name) for name in
            ("code", "message", "retryable", "action", "status", "retry_after_ms", "attempts", "data")}


def wire(position=0, *, sends=3, error=None):
    row = {"session_id": "01J00000000000000000000000", "position": position, "sends": sends,
           "digests": {"q": "0" * 64}}
    row.update(error=error) if error is not None else row.update(response={
        "model": "jev", "answers": {"q": {"type": "noul", "noul": 0.5}},
        "usage": {"input_tokens": 1, "output_tokens": 1}})
    return json.dumps(row)


@pytest.mark.parametrize("category,code,retryable", CATEGORY_CASES)
def test_each_shared_category(consumer, category, code, retryable):
    detail = dict(id="example.failure", category=category, message="typed message", status=429,
                  retry_after_ms=1100, attempts=9, cause={"id": "nested", "message": "detail"})
    detail["recovery"] = "Rust recovery text"
    result = consumer.decode_result(wire(error=detail))
    assert not result["ok"]
    assert result["attempts"] == 3
    assert error_fields(result["error"]) == dict(code=code, message="typed message", retryable=retryable,
                                  action=detail.get("recovery"), status=429,
                                  retry_after_ms=1100, attempts=3, data=detail)


@pytest.mark.parametrize("identifier,category,expected", [
    ("decision.context_exceeded", "validation", "DECIDR.CONTEXT_EXCEEDED"),
    ("decision.api_bad_request", "validation", "DECIDR.VALIDATION"),
    ("decision.api_validation", "validation", "DECIDR.VALIDATION"),
    ("decision.response_parse", "response_invalid", "DECIDR.RESPONSE_INVALID"),
])
def test_each_shared_id(consumer, identifier, category, expected):
    detail = dict(id=identifier, category=category, message="failure")
    result = consumer.decode_result(wire(error=detail))
    assert result["error"].code == expected
    assert result["error"].retryable is (category == "response_invalid")


def test_consumer_overrides_and_wheel_retryability(consumer, monkeypatch):
    monkeypatch.setattr(sc_agent, "retryable", lambda _: True)
    detail = dict(id="decision.context_exceeded", category="validation", message="keep this message")
    detail["recovery"] = "Rust context recovery"
    result = consumer.decode_result(wire(error=detail), category_codes={"context_exceeded": "OWN.CONTEXT"})
    assert result["error"].code == "OWN.CONTEXT"
    assert result["error"].action == "Rust context recovery"
    assert result["error"].retryable is True
    assert result["error"].message == "keep this message"
    # An override need not copy the table to keep the defaults for other categories.
    result = consumer.decode_result(wire(error=dict(id="x", category="log", message="logging failed")), category_codes={"auth": "OWN.AUTH"})
    assert result["error"].code == "DECIDR.LOG"


@pytest.mark.parametrize("value", [True, False, -1, "2", 2.0, None])
def test_metadata_rejects_non_integer_fields(consumer, value):
    assert consumer.error_metadata(dict(status=value, attempts=value, retry_after_ms=value)) == dict(
        status=None, attempts=None, retry_after_ms=None)
    result = consumer.decode_result(wire(sends=value, error=dict(id="x", category="busy", message="busy", attempts=7)))
    assert result["attempts"] is None and result["error"].attempts is None
    assert result["error"].code == "DECIDR.OVERLOADED"


def test_metadata_zero_and_unknown_sends(consumer):
    assert consumer.error_metadata(dict(status=0, attempts=0, retry_after_ms=0)) == dict(
        status=0, attempts=0, retry_after_ms=0)
    result = consumer.decode_result(wire(sends="bad"))
    assert result["ok"] and result["attempts"] is None
    result = consumer.decode_result(wire(sends="bad", error={"id": "x", "category": "busy", "message": "busy", "attempts": 8}))
    assert result["error"].attempts is None
    assert result["error"].code == "DECIDR.OVERLOADED"


@pytest.mark.parametrize("line", ["bad json", "[]", "null", "{}", '{"response":null}', '{"error":true}'])
def test_invalid_result_is_a_plain_failure(consumer, line):
    result = consumer.decode_result(line)
    assert not result["ok"] and result["error"].code == "DECIDR.INVALID_RESULT"
    assert "invalid sc-decidr result" in result["error"].action


def test_sag_error_retains_typed_fields(consumer):
    detail = dict(id="http.network", category="network", message="transport details", attempts=4,
                  retry_after_ms=900, status=503)
    exc = sc_agent.SagError(detail["id"], detail["message"], json.dumps(detail))
    result = consumer.exception_result(exc)
    assert error_fields(result["error"]) == dict(code="DECIDR.NETWORK", message="transport details", retryable=True,
                                   action=detail.get("recovery"), attempts=4,
                                   retry_after_ms=900, status=503, data=detail)


@pytest.mark.parametrize("method,payload", [("call_result", "{}"), ("call_batch_results", ["{}"])])
def test_missing_wheel_has_one_action(consumer, monkeypatch, method, payload):
    monkeypatch.setitem(sys.modules, "sc_agent", None)
    result = getattr(consumer, method)(payload, **COMMON)
    assert result["error"].code == "DEPENDENCY.SC_DECIDR"
    assert "Install sc-decidr" in result["error"].action
    assert result["error"].attempts is None
    with pytest.raises(consumer.ClientError) as caught:
        consumer.new_cancel_token()
    assert caught.value.code == "DEPENDENCY.SC_DECIDR"


@pytest.mark.parametrize("method,operation,payload", [
    ("call_result", "send", "{}"), ("call_batch_results", "send_batch", ["{}"]),
])
@pytest.mark.parametrize("error_kind", ["client", "sag", "unexpected"])
def test_call_failure_paths(consumer, monkeypatch, method, operation, payload, error_kind):
    errors = {
        "client": consumer.ClientError("DECIDR.INVALID_RESULT", "invalid wire"),
        "sag": sc_agent.SagError("auth", "auth details", '{"id":"auth", "message":"auth details", "category":"auth"}'),
        "unexpected": RuntimeError("caller defect"),
    }
    exc = errors[error_kind]

    def fail(*args, **kwargs):
        raise exc

    monkeypatch.setattr(sc_agent, operation, fail)
    cancel = sc_agent.CancelToken()
    if error_kind == "unexpected":
        with pytest.raises(RuntimeError, match="caller defect"):
            getattr(consumer, method)(payload, **COMMON, cancel=cancel)
    else:
        result = getattr(consumer, method)(payload, **COMMON, cancel=cancel)
        assert result["error"].code == ("DECIDR.INVALID_RESULT" if error_kind == "client" else "AUTH.CREDENTIALS")
        assert cancel.is_cancelled() is (error_kind == "sag")


def test_single_helper_forwards_context_and_preserves_wire(consumer, monkeypatch):
    observed = {}
    line = wire(sends=5)

    def send(request, **kwargs):
        observed.update(request=request, **kwargs)
        return line

    monkeypatch.setattr(sc_agent, "send", send)
    log = lambda _: None
    result = consumer.call_result({"model": "jev"}, **COMMON, config_dir="config", keyring_service="service",
                                  keyring_account="account", timeout_secs=30, general_log=log,
                                  images={"image": b"data"}, long_context=True)
    assert result["ok"] and result["value"]["q"].noul == 0.5
    assert result["usage"] == {"input_tokens": 1, "output_tokens": 1}
    assert result["position"] == 0 and result["attempts"] == 5 and result["line"] == line
    assert json.loads(observed["request"]) == {"model": "jev"}
    assert {key: value for key, value in observed.items() if key != "request"} == dict(model="jev", project="consumer", agent_role="judge",
                            config_dir="config", keyring_service="service", keyring_account="account",
                            timeout_secs=30, general_log=log, images={"image": b"data"}, long_context=True,
                            cancel=observed["cancel"])
    assert isinstance(observed["cancel"], sc_agent.CancelToken)


def test_batch_streams_immediately_once_and_returns_input_order(consumer, monkeypatch):
    first, second = wire(0), wire(1, sends=5, error=dict(id="busy", category="busy", message="retry later"))
    callbacks, observed = [], {}

    def send_batch(requests, *, on_result, **kwargs):
        observed.update(requests=requests, **kwargs)
        on_result(second)
        assert [record["position"] for record in callbacks] == [1]
        on_result(first)
        assert [record["position"] for record in callbacks] == [1, 0]
        return [first, second]

    monkeypatch.setattr(sc_agent, "send_batch", send_batch)
    result = consumer.call_batch_results([{"x": "a"}, "b"], **COMMON, on_result=callbacks.append,
                                         config_dir="config", keyring_service="service", keyring_account="account",
                                         timeout_secs=30, workers=2)
    assert result == dict(ok=True, value=[callbacks[1], callbacks[0]])
    assert callbacks[0]["error"].attempts == 5
    assert observed["requests"] == ['{"x": "a"}', "b"]
    assert {key: value for key, value in observed.items() if key != "cancel"} == dict(
        requests=['{"x": "a"}', "b"], model="jev", project="consumer", agent_role="judge", config_dir="config",
        keyring_service="service", keyring_account="account", timeout_secs=30, workers=2, general_log=None)


@pytest.mark.parametrize("position", [None, True, "0", -1, 1])
def test_invalid_batch_position(consumer, monkeypatch, position):
    def send_batch(requests, *, on_result, **kwargs):
        line = wire(position)
        on_result(line)
        return [line]

    monkeypatch.setattr(sc_agent, "send_batch", send_batch)
    result = consumer.call_batch_results(["a"], **COMMON)
    assert result["error"].code == "DECIDR.INVALID_RESULT"


@pytest.mark.parametrize("lines", [[], [wire(0), wire(0)], [wire(0), wire(0, sends=5)]])
def test_invalid_batch_count_or_duplicate_position(consumer, monkeypatch, lines):
    monkeypatch.setattr(sc_agent, "send_batch", lambda *args, **kwargs: lines)
    result = consumer.call_batch_results(["a", "b"], **COMMON)
    assert result["error"].code == "DECIDR.INVALID_RESULT"


def test_interrupted_batch_preserves_callback_and_cancels(consumer, monkeypatch):
    callbacks = []

    def interrupted(requests, *, on_result, **kwargs):
        on_result(wire(1))
        assert len(callbacks) == 1
        raise KeyboardInterrupt

    monkeypatch.setattr(sc_agent, "send_batch", interrupted)
    cancel = sc_agent.CancelToken()
    with pytest.raises(KeyboardInterrupt):
        consumer.call_batch_results(["a", "b"], **COMMON, cancel=cancel, on_result=callbacks.append)
    assert cancel.is_cancelled()
    assert len(callbacks) == 1 and callbacks[0]["position"] == 1


def test_shared_error_is_raisable_and_renders_the_consumer_envelope(consumer):
    error = consumer.ClientError("INPUT.INVALID", "missing field", action="Supply the field.",
                                 status=422, retry_after_ms=700, attempts=0, data={"field": "model"})
    with pytest.raises(consumer.ClientError) as caught:
        raise error
    assert caught.value is error and str(error) == "missing field"
    assert (error.code, error.message, error.retryable, error.action, error.status,
            error.retry_after_ms, error.attempts, error.data) == (
                "INPUT.INVALID", "missing field", False, "Supply the field.", 422, 700, 0, {"field": "model"})
    assert error.to_json() == {"success": False, "data": {"field": "model"}, "error": {
        "code": "INPUT.INVALID", "message": "missing field", "recoverable": False,
        "suggested_action": "Supply the field."}}
    assert not any(hasattr(error, name) for name in ("recoverable", "retry_after", "meta", "with_context"))
    with pytest.raises(AttributeError):
        error.message = "replacement"
    assert consumer.exception_result(error)["error"] is error


@pytest.mark.parametrize("status", [401, 402, 413, 422, 429, 503])
def test_rust_error_text_and_single_log_entry_are_shared_across_consumers(env, args, consumer, status):
    request = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})
    if status == 413:
        env[0].response_body = {"detail": "context window exceeded"}
    if status == 401:
        env[0].response_body = {"detail": "PRIVATE PROVIDER AUTH BODY"}
    common_fields = []
    for project, role in (("plan-anchor", "judge"), ("judge-jev", "quality")):
        records = []
        result = consumer.call_result(request, **dict(args, project=project, agent_role=role),
                                      abort_on=frozenset(), general_log=records.append)
        error = result["error"]
        assert isinstance(error, consumer.ClientError)
        native = json.loads(result["line"])["error"]
        assert error.message == native["message"] and error.action == native["recovery"]
        assert error.action
        envelope = error.to_json()
        assert envelope["error"]["message"] == native["message"]
        assert envelope["error"]["suggested_action"] == native["recovery"]
        if status == 401:
            assert "body withheld" in error.message
            assert "PRIVATE PROVIDER AUTH BODY" not in json.dumps(envelope)
        logged = [record for record in records if record["action"] == "runner.result.error"]
        assert len(logged) == 1
        fields = logged[0]["fields"]
        assert fields["project"] == project and fields["agent_role"] == role
        assert fields["id"] == native["id"] and fields["category"] == native["category"]
        assert fields["status"] == native.get("status") and fields["message"] == native["message"]
        assert fields["action"] == native["recovery"]
        common_fields.append({key: fields[key] for key in ("id", "category", "provider", "status", "message", "action")})
    assert common_fields[0] == common_fields[1]


@pytest.mark.parametrize("batch", [False, True])
def test_each_wire_line_is_parsed_once(consumer, monkeypatch, batch):
    line = wire(error={"id": "decision.context_exceeded", "category": "validation", "message": "too large"})
    parse_result, seen = sc_agent.parse_result, []

    def tracked(value):
        if value == line:
            seen.append(value)
        return parse_result(value)

    def send_batch(requests, *, on_result, **kwargs):
        on_result(line)
        return [line]

    monkeypatch.setattr(sc_agent, "parse_result", tracked)
    monkeypatch.setattr(sc_agent, "send", lambda *args, **kwargs: line)
    monkeypatch.setattr(sc_agent, "send_batch", send_batch)
    result = (consumer.call_batch_results(["request"], **COMMON)["value"][0] if batch else
              consumer.call_result("request", **COMMON))
    assert result["error"].code == "DECIDR.CONTEXT_EXCEEDED"
    assert result["attempts"] == 3
    assert seen == [line]
