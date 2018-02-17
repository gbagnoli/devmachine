#!/bin/bash

set -eu

pushd "$(dirname "$GIT_DIR")" >/dev/null
bundle check >/dev/null || bundle install
bundle exec rubocop site-cookbooks
bundle exec foodcritic site-cookbooks
popd >/dev/null
