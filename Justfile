set windows-shell := ["cmd.exe", "/c"]
python := if os_family() == "windows" { "python" } else { "python3" }

integration *args:
    {{python}} scripts/integration.py {{args}}
