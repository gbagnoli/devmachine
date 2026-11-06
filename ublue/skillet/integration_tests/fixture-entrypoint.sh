#!/bin/sh
set -eu
cp /fixture/config /data/observed-config
sha256sum /run/secrets/skillet-smoke-dummy | cut -d' ' -f1 > /data/observed-secret-sha
while :; do sleep 3600; done
