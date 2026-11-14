#!/usr/bin/env python3
"""Where a judge-jev install lives, and which locations are global.

One rule, shared by the installer, the gates and the settings lookup: walk up
from the working directory, nearest first, and stop before the home directory.

  - ``ancestors(start)``: ``start`` and its parents, nearest first. When
    ``start`` is under the home directory the walk stops before it (the home
    directory itself is never visited); otherwise it ends at the filesystem
    root. ``start`` equal to the home directory visits nothing. When the home
    directory cannot be determined (Windows with no ``USERPROFILE`` or
    ``HOMEDRIVE``/``HOMEPATH``), the walk ends at the filesystem root.
  - ``find_installs(start)``: the nearest directory holding
    ``<dir>/<harness dir>/skills/judge-jev/scripts/jev_judge.py`` and every
    copy in it (one per harness directory the installer writes: .claude,
    .codex, .cursor; never .agents, which the installer does not write).
  - ``trusted_script(script, start)``: the script must be one of those copies
    and the install root must hold the installer's
    ``.sc/judge-jev/install-lock.json`` naming it; a hand-copied or lock-less
    copy is refused.
  - ``find_settings(start)`` / ``find_lock(start)``: the nearest
    ``.sc/judge-jev/settings.yaml`` / ``install-lock.json``.
  - ``global_location(path)``: why ``path`` is a global (user-wide or
    machine-wide) harness location, or None. The home directory itself, and
    anything inside ``~/.claude``, ``~/.codex``, ``~/.cursor``, ``~/.agents``,
    ``/etc/codex`` (Codex admin skills) and the Claude Code managed-settings
    directories, is global; that covers every plugin cache
    under them. A judge-jev install is per folder, never global. With no
    determinable home directory only the machine-wide directories are known
    to be global, and they are still refused.

  - ``central_log_root(start)``: ``~/.judge-jev/<repository name>/<UTC date>``,
    the per-user folder every judge-jev run of a repository logs under, whatever
    worktree or scratch directory it ran in. ``repository_name(start)`` is the
    name of the main checkout: a linked worktree reports the checkout it belongs
    to. None when the home directory is unknown.

No lookup raises because the home directory is unknown.

Standard library only; importing it has no side effects.
"""

from __future__ import annotations

import datetime as _dt
import json
import os
import re
from pathlib import Path
from typing import Iterator, List, Optional, Tuple

SKILL_NAME = "judge-jev"
HARNESS_DIRS = (".claude", ".codex", ".cursor")  # the directories install.py writes (--target)
# User-wide harness directories: global, so never an install location. .agents (Codex's documented
# user skill directory) is global here but is not a harness directory the installer writes.
GLOBAL_HARNESS_DIRS = HARNESS_DIRS + (".agents",)
SETTINGS_RELPATH = ".sc/judge-jev/settings.yaml"
LOCK_RELPATH = ".sc/judge-jev/install-lock.json"
SCRIPT_RELPATH = f"skills/{SKILL_NAME}/scripts/jev_judge.py"
# Machine-wide skill/config directories (Codex admin skills; Claude Code managed settings).
SYSTEM_GLOBALS = (
    "/etc/codex",
    "/etc/claude-code",
    "/Library/Application Support/ClaudeCode",
    "C:/ProgramData/ClaudeCode",
)


def _expand(path: Path) -> Path:
    """``path`` with a leading ``~`` expanded; unchanged when the home directory cannot be determined."""
    try:
        return Path(path).expanduser()
    except (KeyError, RuntimeError):
        return Path(path)


def _resolve(path: Path) -> Path:
    expanded = _expand(path)
    try:
        return expanded.resolve()
    except (OSError, RuntimeError):
        return Path(os.path.abspath(str(expanded)))


def home_dir() -> Optional[Path]:
    """The user's home directory (``$HOME`` / ``%USERPROFILE%``), resolved, or None when it cannot be determined.

    ``os.path.expanduser`` returns ``~`` unchanged when no home is known (on
    Windows: no ``USERPROFILE`` and no ``HOMEDRIVE``/``HOMEPATH``; on POSIX: no
    ``HOME`` and no password-database entry), and ``Path.home()`` raises.
    """
    try:
        raw = os.path.expanduser("~")
    except (KeyError, RuntimeError, OSError):
        return None
    if not raw or raw.startswith("~"):
        return None
    return _resolve(Path(raw))


def _home(home: Optional[Path]) -> Optional[Path]:
    return _resolve(home) if home is not None else home_dir()


def _within(path: Path, root: Path) -> bool:
    return path == root or root in path.parents


def global_roots(home: Optional[Path] = None) -> List[Path]:
    """The global directories: the harness directories under home (when it is known) and the machine-wide ones."""
    home = _home(home)
    roots = [home / name for name in GLOBAL_HARNESS_DIRS] if home is not None else []
    roots += [_resolve(Path(p)) for p in SYSTEM_GLOBALS]
    return roots


def global_location(path: Path, home: Optional[Path] = None) -> Optional[str]:
    """Why ``path`` is a global location (a short reason), or None when it is a per-folder location."""
    home = _home(home)
    target = _resolve(path)
    if home is not None and target == home:
        return f"{home} is the home directory"
    for root in global_roots(home):
        if _within(target, root):
            return f"{target} is inside the global location {root}"
    return None


def ancestors(start: Path, home: Optional[Path] = None) -> Iterator[Path]:
    """``start`` and its parents, nearest first, stopping before the home directory (if it is known)."""
    home = _home(home)
    current = _resolve(start)
    if current.is_file():
        current = current.parent
    if home is not None and current == home:
        return
    under_home = home is not None and home in current.parents
    while True:
        yield current
        parent = current.parent
        if parent == current or (under_home and parent == home):
            return
        current = parent


def find_installs(start: Path, home: Optional[Path] = None) -> Optional[Tuple[Path, List[Path]]]:
    """(install root, [jev_judge.py per harness dir]) for the nearest install above ``start``, or None."""
    for directory in ancestors(start, home):
        found = [directory / harness / SCRIPT_RELPATH for harness in HARNESS_DIRS]
        found = [p for p in found if p.is_file()]
        if found:
            return directory, found
    return None


def _nearest(start: Path, rel: str, home: Optional[Path]) -> Optional[Path]:
    for directory in ancestors(start, home):
        candidate = directory.joinpath(*rel.split("/"))
        if candidate.exists() or candidate.is_symlink():
            return candidate
    return None


def find_settings(start: Path, home: Optional[Path] = None) -> Optional[Path]:
    """The nearest ``.sc/judge-jev/settings.yaml`` at or above ``start`` (stopping before home)."""
    return _nearest(start, SETTINGS_RELPATH, home)


def find_lock(start: Path, home: Optional[Path] = None) -> Optional[Path]:
    """The nearest ``.sc/judge-jev/install-lock.json`` at or above ``start`` (stopping before home)."""
    return _nearest(start, LOCK_RELPATH, home)


def trusted_script(script: Path, start: Path, home: Optional[Path] = None) -> Tuple[bool, str]:
    """Whether ``script`` is the jev_judge.py the trust rule selects from ``start``; (ok, reason)."""
    resolved = _resolve(script)
    why = global_location(resolved, home)
    if why is not None:
        return False, f"the script is in a global location ({why}); judge-jev is installed per folder"
    nearest = find_installs(start, home)
    if nearest is None:
        return False, (f"no {', '.join(HARNESS_DIRS)} install of {SKILL_NAME} at or above {_resolve(start)} "
                       "(the walk stops before the home directory)")
    root, copies = nearest
    if resolved not in {_resolve(p) for p in copies}:
        return False, f"the nearest install is {root}; {resolved} is not one of its copies"
    named, why = lock_names(root, resolved)
    if not named:
        return False, why
    return True, f"nearest install {root}"


def lock_names(root: Path, script: Path) -> Tuple[bool, str]:
    """Whether ``root``'s install lock (written by install.py) names ``script``; (ok, reason).

    Only an install the installer wrote is trusted: a copy without the lock, or one the lock does not list,
    was placed by hand.
    """
    lock = root.joinpath(*LOCK_RELPATH.split("/"))
    try:
        data = json.loads(lock.read_text(encoding="utf-8"))
    except FileNotFoundError:
        return False, (f"{root} has no {LOCK_RELPATH}: the copy was not written by the judge-jev installer "
                       "(install.py --dest <folder>)")
    except (OSError, UnicodeError, ValueError) as exc:
        return False, f"{lock} could not be read as JSON ({type(exc).__name__})"
    files = data.get("files") if isinstance(data, dict) else None
    if not isinstance(files, dict):
        return False, f"{lock} is not a judge-jev install record"
    try:
        key = _resolve(script).relative_to(_resolve(root)).as_posix()
    except ValueError:
        return False, f"{script} is not inside the install root {root}"
    if key not in files:
        return False, f"{lock} does not list {key}: the copy was not written by the judge-jev installer"
    return True, f"{lock} lists {key}"


CENTRAL_LOG_DIRNAME = ".judge-jev"


def repository_name(start: Path, home: Optional[Path] = None) -> str:
    """The name of the repository ``start`` is in: the folder of its main checkout.

    The nearest ancestor holding ``.git`` decides. A ``.git`` directory is a main checkout: its folder's name. A ``.git``
    file is a linked worktree (``gitdir: <main>/.git/worktrees/<name>``): the name of ``<main>``, so every worktree of a
    repository shares one name. With no ``.git`` above ``start`` the name is ``start``'s own. Characters outside
    ``A-Za-z0-9._-`` become ``_``.
    """
    origin = _resolve(start)
    if origin.is_file():
        origin = origin.parent
    chosen = origin
    for directory in ancestors(origin, home):
        marker = directory / ".git"
        if marker.is_dir():
            chosen = directory
            break
        if marker.is_file():
            chosen = directory
            try:
                text = marker.read_text(encoding="utf-8", errors="replace")
            except OSError:
                break
            match = re.match(r"gitdir:\s*(.+?)\s*$", text, re.MULTILINE)
            if match:
                parts = Path(match.group(1)).parts
                if ".git" in parts:
                    main = Path(*parts[:parts.index(".git")])
                    if main.name:
                        chosen = main
            break
    name = re.sub(r"[^A-Za-z0-9._-]", "_", chosen.name).strip(".")
    return name or "repository"


def central_log_root(start: Path, home: Optional[Path] = None, today: Optional[_dt.date] = None) -> Optional[Path]:
    """``~/.judge-jev/<repository name>/<UTC date>``: where a judge-jev run of the repository at ``start`` logs by default.
    None when the home directory cannot be determined."""
    home = _home(home)
    if home is None:
        return None
    day = today or _dt.datetime.now(_dt.timezone.utc).date()
    return home / CENTRAL_LOG_DIRNAME / repository_name(start, home) / day.isoformat()
