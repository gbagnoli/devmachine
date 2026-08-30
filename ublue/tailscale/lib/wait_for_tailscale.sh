#!/bin/bash
WAIT_TIME=1
CONNECTED=0
MAX_ATTEMPTS=30

echo "Waiting for tailscale to connect..." >&2

for (( i=1; i<=MAX_ATTEMPTS; i++ )); do
    if tailscale status --json 2>/dev/null | jq -e 'select(.BackendState == "Running")' > /dev/null; then
        CONNECTED=1
        echo "Tailscale is connected." >&2
        break
    fi

    echo "Attempt $i/$MAX_ATTEMPTS: Tailscale not yet connected. Waiting $WAIT_TIME seconds..." >&2
    sleep $WAIT_TIME
    WAIT_TIME=$(( WAIT_TIME * 2 ))
done

if [ "$CONNECTED" -eq 0 ]; then
    echo "Error: Tailscale failed to connect after $MAX_ATTEMPTS attempts." >&2
    exit 1
fi
