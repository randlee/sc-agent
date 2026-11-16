"""The installed public type stub covers the actual native exports."""
import ast
import importlib.resources
import inspect

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
