#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
chef exec cookstyle site-cookbooks roles
uv run ruff check --diff
uv run ruff format --diff
popd >/dev/null
