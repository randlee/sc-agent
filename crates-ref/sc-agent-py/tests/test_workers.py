"""Worker bounds are checked before sending."""
import json

import pytest
import sc_agent

NOUL = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})


@pytest.mark.parametrize("workers", [0, -1, 65])
def test_invalid_workers_are_typed_before_send(env, args, workers):
    loopback, _ = env
    with pytest.raises(sc_agent.SagError) as caught:
        sc_agent.send_batch([NOUL], **args, workers=workers)
    assert caught.value.id == "decidr.invalid_workers"
    assert caught.value.category == "validation"
    assert str(workers) in caught.value.message
    assert not loopback.requests


def test_single_send_has_no_workers_argument(env, args):
    with pytest.raises(TypeError):
        sc_agent.send(NOUL, **args, workers=1)


@pytest.mark.mode("ok")
@pytest.mark.parametrize("workers", [3, 9])
def test_worker_override_sets_worker_and_queue_counts(env, args, workers):
    events = []
    lines = sc_agent.send_batch([NOUL], **args, workers=workers, general_log=events.append)
    assert len(lines) == 1
    starts = [event for event in events if event.get("action") == "runner.stream.start"]
    assert len(starts) == 1
    assert starts[0]["fields"]["workers"] == workers
    assert starts[0]["fields"]["queue_capacity"] == 4 * workers
