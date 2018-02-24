#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
bundle check >/dev/null || bundle install
bundle exec rubocop site-cookbooks roles
bundle exec foodcritic -B site-cookbooks -R roles
popd >/dev/null
