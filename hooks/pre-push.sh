#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
chef exec cookstyle site-cookbooks roles
black --check fabfile.py
isort fabfile.py
popd >/dev/null
