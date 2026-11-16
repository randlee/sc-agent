"""Drive queued Python log callbacks through the real interpreter-exit hook."""
import json
import subprocess
import sys
import textwrap

import pytest

from conftest import HANG_GUARD


@pytest.mark.mode("ok")
def test_queued_python_logs_detach_before_interpreter_exit(env, args):
    child = textwrap.dedent(r'''
        import atexit
        import json
        import sys
        import threading
        import warnings
        import sc_agent

        args = json.loads(sys.argv[1])
        guard = float(sys.argv[2])
        request = json.dumps({"model": "jev-latest", "state": "s", "questions": {"q": {"type": "noul"}}})
        entered, release, queued, finished = (threading.Event() for _ in range(4))
        records, failures, results = [], [], []
        completed = []
        count = 64

        def sink(record):
            entered.set()
            if not release.wait(guard):
                failures.append("callback release was not signalled")
                raise RuntimeError(failures[-1])
            records.append(record)
            finished.set()

        def on_result(line):
            completed.append(line)
            if len(completed) == count:
                queued.set()

        def run():
            try:
                results.extend(sc_agent.send_batch([request] * count, **args,
                    session_log=sink, on_result=on_result))
            except BaseException as error:
                failures.append(error)

        with warnings.catch_warnings(record=True) as refused:
            warnings.simplefilter("always", RuntimeWarning)
            worker = threading.Thread(target=run)
            worker.start()
            try:
                assert entered.wait(guard), "no callback entered"
                assert queued.wait(guard), "not all session events were queued"
                # LIFO: release the held callback, then run sc-agent's actual
                # registered shutdown hook while admitted events remain queued.
                atexit.register(release.set)
                atexit._run_exitfuncs()
                assert finished.is_set(), "exit hook returned before the active callback finished"
                before = len(records)
                worker.join(guard)
                assert not worker.is_alive(), "batch did not return after shutdown"
                assert len(completed) == count
                assert all("response" in json.loads(line) for line in completed)
                # Shutdown may detach queued session rows. Their refusal must be
                # typed and visible, never a crash or a silently lost record.
                for error in failures:
                    assert isinstance(error, sc_agent.SagError), repr(error)
                    assert error.category == "log"
                    assert "log.rows_lost" in error.detail
                if not failures:
                    assert len(results) == count
                # A new bridge must refuse every Python callback after shutdown.
                with warnings.catch_warnings(record=True) as closed:
                    warnings.simplefilter("always", RuntimeWarning)
                    try:
                        result = sc_agent.send(request, **args, session_log=sink)
                    except sc_agent.SagError as error:
                        assert error.category == "log"
                        assert "log.rows_lost" in error.detail
                    else:
                        assert "response" in json.loads(result)
                        assert any("log.rows_lost" in str(w.message) for w in closed)
                assert len(records) == before, "callback admitted after the exit hook"
            finally:
                release.set()
                worker.join(guard)
        print(json.dumps({"queued": count, "callbacks_after_shutdown": len(records) - before}))
    ''')
    result = subprocess.run(
        [sys.executable, "-c", child, json.dumps(args), str(HANG_GUARD)],
        capture_output=True, text=True, timeout=HANG_GUARD * 3,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    assert json.loads(result.stdout) == {"queued": 64, "callbacks_after_shutdown": 0}
