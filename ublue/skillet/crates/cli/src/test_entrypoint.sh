#!/bin/sh
set -eu
round=$1
mock=/tmp/skillet-test-mocks
state=/tmp/skillet-test-state
mkdir -p "$mock" "$state"
if [ ! -x "$mock/systemctl" ]; then
  cat >"$mock/systemctl" <<'MOCK'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> /tmp/skillet-test.systemctl.log
case "$1" in
  is-active) [ -f "/tmp/skillet-test-state/active-$3" ] ;;
  start|restart) touch "/tmp/skillet-test-state/active-$2" ;;
  enable|daemon-reload|stop|reload) exit 0 ;;
  *) exit 2 ;;
esac
MOCK
  cat >"$mock/podman" <<'MOCK'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> /tmp/skillet-test.podman.log
case "$1 $2" in
  "secret exists") [ -f /tmp/skillet-test-state/secret ] ;;
  "secret inspect")
    case "$*" in *'{{.ID}}'*) printf 'skillet-test-secret-id\n' ;; *) cat /tmp/skillet-test-state/secret-hash ;; esac
    ;;
  "secret create")
    shift 2
    hash=
    while [ "$#" -gt 0 ]; do
      case "$1" in --replace) shift ;; --label) hash=${2#skillet.payload_hash=}; shift 2 ;; *) break ;; esac
    done
    name=$1
    cat > /tmp/skillet-test-state/secret-payload
    printf '%s\n' "$hash" > /tmp/skillet-test-state/secret-hash
    touch /tmp/skillet-test-state/secret
    ;;
  *) exit 2 ;;
esac
MOCK
  chmod +x "$mock/systemctl" "$mock/podman"
fi
export PATH="$mock:$PATH"
if [ "$round" = second ]; then printf '\nSECOND\n' >> /tmp/skillet-test.systemctl.log; fi
/usr/bin/skillet apply
