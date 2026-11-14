"""Generic installer regression with real offline wheels/venv and controlled binary versions."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import zipfile

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("install_wheels", ROOT / "scripts/install_wheels.py")
helper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(helper)
VERSION = "1.2.3"


def wheel(path, package, module):
    dist = package.replace("-", "_")
    info = f"{dist}-{VERSION}.dist-info"
    files = {
        f"{module}.py": '"""Controlled install fixture; no native code or credential access."""\nVALUE = 17\n',
        f"{info}/METADATA": f"Metadata-Version: 2.1\nName: {package}\nVersion: {VERSION}\n",
        f"{info}/WHEEL": "Wheel-Version: 1.0\nGenerator: installer-unit-test\nRoot-Is-Purelib: true\nTag: py3-none-any\n",
    }
    files[f"{info}/RECORD"] = "".join(f"{name},,\n" for name in [*files, f"{info}/RECORD"])
    target = path / f"{dist}-{VERSION}-py3-none-any.whl"
    with zipfile.ZipFile(target, "w") as archive:
        for name, content in files.items():
            archive.writestr(name, content)
    return target


class WheelInstall(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="wheel installer ")
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name)
        cls.stage = cls.root / "stage with spaces"
        (cls.root / "release").mkdir()
        (cls.root / "release/publish-artifacts.toml").write_text(
            '[prerelease]\nbinaries = ["decidr", "sitr"]\n'
            '[[python_packages]]\npackage = "sc-decidr"\nmodule = "sc_agent"\n'
            '[[python_packages]]\npackage = "sitr"\nmodule = "sc_treesitter"\n'
        )
        wheels = [wheel(cls.root, "sc-decidr", "sc_agent"), wheel(cls.root, "sitr", "sc_treesitter")]
        with mock.patch.object(sys, "argv", ["install_wheels", "--stage", str(cls.stage), *map(str, wheels)]):
            helper.main()  # real venv creation and pip --no-index --no-deps
        suffix = ".exe" if sys.platform == "win32" else ""
        cls.binaries = [cls.stage / "bin" / f"{name}{suffix}" for name in ("decidr", "sitr")]
        for binary in cls.binaries:
            binary.parent.mkdir(exist_ok=True)
            binary.touch()  # only --version below is controlled; these files never execute

    def verify(self, version=VERSION, binary_version=VERSION):
        real_run = subprocess.run
        binary_calls = []
        def controlled(command, **kwargs):
            if Path(command[0]) in self.binaries:
                self.assertEqual(command[1:], ["--version"])
                binary_calls.append(Path(command[0]))
                return subprocess.CompletedProcess(command, 0, stdout=f"tool {binary_version}\n", stderr="")
            return real_run(command, **kwargs)  # actual installed interpreter/import/metadata check
        with mock.patch.object(helper, "ROOT", self.root), \
                mock.patch.object(helper.subprocess, "run", side_effect=controlled), \
                mock.patch.object(sys, "argv", ["install_wheels", "--verify", "--stage", str(self.stage), "--version", version]), \
                contextlib.redirect_stdout(io.StringIO()) as output:
            helper.main()
        return binary_calls, output.getvalue()

    def test_install_and_verify_imports_the_declared_wheels(self):
        calls, output = self.verify()
        self.assertEqual(calls, self.binaries)
        self.assertEqual(output.strip(), VERSION)

    def test_binary_version_mismatch_is_refused(self):
        with self.assertRaisesRegex(SystemExit, "version differs"):
            self.verify(binary_version="0.0.1")

    def test_actual_installed_wheel_version_mismatch_is_refused(self):
        with self.assertRaises(subprocess.CalledProcessError):
            self.verify(version="9.9.9", binary_version="9.9.9")

    def test_install_requires_wheel_arguments(self):
        with mock.patch.object(sys, "argv", ["install_wheels", "--stage", str(self.stage)]):
            with self.assertRaises(SystemExit) as caught:
                helper.main()
        self.assertEqual(caught.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
