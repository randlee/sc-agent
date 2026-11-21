"""Batch return order is independent of completion order."""
import json
import threading

import pytest
import sc_agent


@pytest.mark.mode("ordered")
def test_batch_returns_input_order_while_callback_streams_completions(env, args):
    loopback, _ = env
    requests = [json.dumps({"model": "jev-latest", "state": str(i), "questions": {"q": {"type": "noul"}}}) for i in range(3)]
    callbacks = []
    arrived = [threading.Event() for _ in range(3)]
    result = []
    errors = []

    def callback(line):
        position = json.loads(line)["position"]
        callbacks.append(position)
        arrived[position].set()

    def call():
        try:
            result.extend(sc_agent.send_batch(requests, **args, workers=3, on_result=callback))
        except BaseException as error:
            errors.append(error)

    thread = threading.Thread(target=call)
    thread.start()
    try:
        for event in loopback.ordered_started:
            assert event.wait(60), "request did not start"
        for i in (2, 1, 0):
            loopback.ordered_release[i].set()
            assert arrived[i].wait(60), "callback did not arrive"
    finally:
        for event in loopback.ordered_release:
            event.set()
        thread.join(60)
    assert not thread.is_alive()
    assert not errors
    assert callbacks == [2, 1, 0]
    assert [json.loads(line)["position"] for line in result] == [0, 1, 2]
