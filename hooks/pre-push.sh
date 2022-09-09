#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
chef exec cookstyle site-cookbooks roles
pipenv run -- black --check fabfile.py
pipenv run -- isort fabfile.py
popd >/dev/null
