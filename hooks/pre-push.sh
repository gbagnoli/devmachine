#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
cinc exec cookstyle site-cookbooks roles
uv run ruff check --diff .
uv run ruff format --diff .
uv run mypy --ignore-missing-imports site-cookbooks fabfile.py
find . -name "*.sh" -not -path "./.venv/*" -not -path "./berks-cookbooks/*" -exec shellcheck -x {} +
shellcheck -x run
popd >/dev/null
