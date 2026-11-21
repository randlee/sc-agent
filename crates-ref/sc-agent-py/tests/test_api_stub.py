"""The installed public type stub covers the actual native exports."""
import ast
import importlib.resources
import inspect
import json

import sc_agent


def test_installed_wheel_contains_pep561_marker():
    assert (importlib.resources.files(sc_agent) / "py.typed").is_file()


def test_stub_covers_native_public_exports():
    tree = ast.parse((importlib.resources.files(sc_agent) / "__init__.pyi").read_text())
    declared = {node.name for node in tree.body if isinstance(node, (ast.FunctionDef, ast.ClassDef))}
    exported = {name for name in vars(sc_agent._native) if not name.startswith("_")}
    assert declared == exported
    for name in ("models", "doctor"):
        stub = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == name)
        native = inspect.signature(getattr(sc_agent._native, name))
        assert [arg.arg for arg in stub.args.args] == list(native.parameters)
        assert all(isinstance(value, ast.Constant) and value.value is None for value in stub.args.defaults)
        assert all(p.default is None for p in native.parameters.values())
        assert isinstance(stub.returns, ast.Name) and stub.returns.id == "str"
    for name in ("NoulAnswer", "ChoiceAnswer", "ScoreAnswer", "BoxAnswer", "parse_result"):
        assert name in declared


def test_parse_result_returns_typed_read_only_answers():
    response = {
        "model": "jev",
        "answers": {
            "n": {"type": "noul", "noul": 0.8},
            "c": {"type": "choice", "choice": "yes", "confidence": 0.9,
                  "probabilities": {"yes": 0.9, "no": 0.1}},
            "s": {"type": "score", "score": 2, "confidence": 0.75,
                  "legend": {"0": "low", "2": "high"}, "probabilities": {"2": 0.75}},
            "b": {"type": "box", "box": {"top": 0.1, "bottom": 0.9, "left": 0.2, "right": 0.8},
                  "confidence": 0.7},
        },
        "usage": {"input_tokens": 12, "output_tokens": 4},
    }
    result = sc_agent.parse_result(json.dumps({
        "session_id": "01J00000000000000000000000", "position": 2, "sends": 3,
        "digests": {"n": "0" * 64}, "response": response,
    }))
    assert result["position"] == 2 and result["sends"] == 3
    assert result["usage"] == {"input_tokens": 12, "output_tokens": 4}
    assert isinstance(result["answers"]["n"], sc_agent.NoulAnswer)
    assert result["answers"]["n"].noul == 0.8
    assert isinstance(result["answers"]["c"], sc_agent.ChoiceAnswer)
    assert result["answers"]["c"].probabilities["yes"] == 0.9
    try:
        result["answers"]["c"].probabilities["yes"] = 0.1
    except TypeError:
        pass
    else:
        raise AssertionError("typed answer mappings must be read-only")
    assert isinstance(result["answers"]["s"], sc_agent.ScoreAnswer)
    assert result["answers"]["s"].legend["2"] == "high"
    assert isinstance(result["answers"]["b"], sc_agent.BoxAnswer)
    assert result["answers"]["b"].right == 0.8
    try:
        result["answers"]["n"].noul = 0.1
    except AttributeError:
        pass
    else:
        raise AssertionError("typed answer fields must be read-only")
