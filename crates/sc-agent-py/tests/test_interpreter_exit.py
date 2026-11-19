"""Direct Python session callbacks finish before the wheel returns and Python exits."""
import json
import subprocess
import sys
import textwrap

import pytest
from conftest import HANG_GUARD


@pytest.mark.mode("ok")
def test_direct_callbacks_finish_before_interpreter_exit(env, args):
    child = textwrap.dedent(r'''
        import atexit
        import json
        import sys
        import threading
        import sc_agent

        args = json.loads(sys.argv[1])
        guard = float(sys.argv[2])
        request = json.dumps({"model":"jev-latest","state":"s","questions":{"q":{"type":"noul"}}})
        entered, release = threading.Event(), threading.Event()
        records, results, errors = [], [], []
        def sink(record):
            entered.set()
            assert release.wait(guard), "callback release missing"
            records.append(record)
        def run():
            try:
                results.extend(sc_agent.send_batch([request] * 8, **args, session_log=sink))
            except BaseException as error:
                errors.append(error)
        worker = threading.Thread(target=run)
        worker.start()
        try:
            assert entered.wait(guard), "callback never entered"
            assert not records and not results
        finally:
            release.set()
            worker.join(guard)
        assert not worker.is_alive() and not errors, errors
        assert len(records) == len(results) == 8
        assert all("response" in json.loads(line) for line in results)
        assert sorted(row["fields"]["position"] for row in records) == list(range(8))
        def verify_exit():
            assert len(records) == 8
            print(json.dumps({"callbacks_completed_before_exit": len(records)}))
        atexit.register(verify_exit)
    ''')
    result = subprocess.run([sys.executable, "-c", child, json.dumps(args), str(HANG_GUARD)],
                            stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=HANG_GUARD * 2)
    assert result.returncode == 0, result.stdout + result.stderr
    assert json.loads(result.stdout) == {"callbacks_completed_before_exit": 8}
