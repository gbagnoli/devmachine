#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
if command -v cinc >/dev/null 2>&1 && cinc --version >/dev/null 2>&1; then
  cinc exec cookstyle site-cookbooks roles
else
  echo "Skipping Cookstyle: cinc is not installed" >&2
fi
uv run ruff check --diff .
uv run ruff format --diff .
uv run mypy --ignore-missing-imports site-cookbooks fabfile.py
git ls-files -z -- "*.sh" | xargs -0 -r shellcheck -x
shellcheck -x run
popd >/dev/null
