set windows-shell := ["cmd.exe", "/c"]
python := if os_family() == "windows" { "python" } else { "python3" }

integration *args:
    {{python}} scripts/integration.py {{args}}

# sc-lint consumer recipes (sc-lint docs just-setup). The generated `default: lint`
# is omitted so bare `just` keeps running `integration`. sc-lint.toml declares no
# lint or test steps yet.

setup:
    {{python}} .sc-lint/bootstrap.py setup --config sc-lint.toml

lint selector='' *scope:
    {{python}} .sc-lint/bootstrap.py lint --config sc-lint.toml "{{selector}}" {{scope}}

test *layer:
    {{python}} .sc-lint/bootstrap.py test --config sc-lint.toml {{layer}}

upgrade:
    {{python}} .sc-lint/bootstrap.py upgrade --config sc-lint.toml
