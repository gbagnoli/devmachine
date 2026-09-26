#!/usr/bin/env bash
set -euo pipefail

phase=${1:?phase required}
binary=/var/lib/skillet/skillet-smoke
clamps_binary=${2:?clamps binary path required}
root=/var/lib/skillet-smoke
service=skillet-smoke-fixture.service
container=skillet-smoke-fixture

fail() {
  echo "smoke failed: $*" >&2
  systemctl --no-pager status "$service" >&2 || true
  journalctl -u "$service" -n 30 --no-pager >&2 || true
  exit 1
}
trap 'fail "unexpected command at line $LINENO"' ERR

apply() {
  timeout 180s "$binary" apply --host skillet-smoke || fail "fixture apply"
}

running() {
  systemctl is-active --quiet "$service" || fail "service is inactive"
  podman inspect "$container" >/dev/null || fail "container is missing"
}

wait_running() {
  local end=$((SECONDS + 60))
  while (( SECONDS < end )); do
    if systemctl is-active --quiet "$service" && podman inspect "$container" >/dev/null 2>&1; then
      return
    fi
    sleep 1
  done
  fail "service did not start after reboot"
}

observed() {
  local wanted=$1
  local end=$((SECONDS + 30))
  while (( SECONDS < end )); do
    if [[ -f "$root/data/observed-config" ]] && cmp -s "$root/data/observed-config" "$root/desired/config"; then
      if [[ $wanted == config ]]; then return; fi
      if [[ -f "$root/data/observed-secret-sha" ]] &&
        [[ $(sha256sum "$root/desired/secret" | cut -d ' ' -f 1) == $(cat "$root/data/observed-secret-sha") ]]; then
        return
      fi
    fi
    sleep 1
  done
  fail "fixture did not consume desired $wanted"
}

snapshot() {
  sha256sum /etc/skillet-smoke/config /etc/skillet-smoke/entrypoint.sh /etc/containers/systemd/skillet-smoke-fixture.container
  stat -c '%n %a %u %g' /etc/skillet-smoke /etc/skillet-smoke/config /etc/skillet-smoke/entrypoint.sh /etc/containers/systemd/skillet-smoke-fixture.container "$root/data"
  systemctl show "$service" -p ActiveState -p Result
  podman inspect --format '{{.Id}} {{.State.StartedAt}}' "$container"
}

case "$phase" in
  before-reboot)
    # The smoke fixture is namespaced and the target was explicitly opted in
    # as disposable. Reset only this fixture so `test smoke` can be rerun on
    # the same VM after inspecting a previous run.
    systemctl stop "$service" >/dev/null 2>&1 || true
    systemctl disable "$service" >/dev/null 2>&1 || true
    podman rm -f "$container" >/dev/null 2>&1 || true
    rm -rf "$root" /etc/skillet-smoke /etc/containers/systemd/skillet-smoke-fixture.container
    systemctl daemon-reload
    mkdir -p "$root/desired" "$root/data"
    touch "$root/allow-start"
    printf 'revision-one\n' > "$root/desired/config"
    printf 'dummy-secret-one\n' > "$root/desired/secret"
    chmod 0600 "$root/desired/secret"
    timeout 180s podman pull docker.io/library/alpine:3.20 >/dev/null || fail "fixture image pull"

    timeout 120s "$clamps_binary" apply --phase base || fail "base apply"
    if env -u CREDENTIALS_DIRECTORY timeout 120s "$clamps_binary" apply >"$root/full-apply.stdout" 2>"$root/full-apply.stderr"; then
      fail "full clamps apply accepted missing credentials"
    fi
    if ! grep -Eq 'CREDENTIALS_DIRECTORY|credential' "$root/full-apply.stderr"; then
      fail "full clamps apply failed for an unexpected reason"
    fi

    apply
    running
    observed secret
    printf 'persistent-test-data\n' > "$root/data/sentinel"
    snapshot > "$root/first.snapshot"
    apply
    running
    observed secret
    snapshot > "$root/repeat.snapshot"
    cmp -s "$root/first.snapshot" "$root/repeat.snapshot" || fail "repeat apply changed managed state or container identity"

    systemctl stop "$service" || fail "stopping fixture"
    apply
    running
    observed secret

    printf 'revision-two\n' > "$root/desired/config"
    apply
    running
    observed secret
    snapshot > "$root/config-change.snapshot"
    apply
    snapshot > "$root/config-repeat.snapshot"
    cmp -s "$root/config-change.snapshot" "$root/config-repeat.snapshot" || fail "configuration repeat recreated container"

    printf 'dummy-secret-two\n' > "$root/desired/secret"
    apply
    running
    observed secret
    snapshot > "$root/secret-change.snapshot"
    apply
    snapshot > "$root/secret-repeat.snapshot"
    cmp -s "$root/secret-change.snapshot" "$root/secret-repeat.snapshot" || fail "secret repeat recreated container"

    systemctl stop "$service" || fail "stopping fixture before startup failure"
    rm "$root/allow-start"
    if timeout 120s "$binary" apply --host skillet-smoke >"$root/startup-failure.stdout" 2>"$root/startup-failure.stderr"; then
      fail "startup failure returned success"
    fi
    systemctl show "$service" -p ActiveState -p Result > "$root/startup-failure.systemd"
    journalctl -u "$service" -n 30 --no-pager > "$root/startup-failure.journal"
    grep -q 'Result=exit-code' "$root/startup-failure.systemd" || fail "startup failure did not report the injected exit code"
    touch "$root/allow-start"
    apply
    running
    observed secret

    rm "$root/allow-start"
    printf 'revision-three\n' > "$root/desired/config"
    if timeout 120s "$binary" apply --host skillet-smoke >"$root/interrupted.stdout" 2>"$root/interrupted.stderr"; then
      fail "interrupted activation returned success"
    fi
    systemctl show "$service" -p ActiveState -p Result > "$root/interrupted.systemd"
    journalctl -u "$service" -n 30 --no-pager > "$root/interrupted.journal"
    grep -q 'Result=exit-code' "$root/interrupted.systemd" || fail "interrupted activation did not report the injected exit code"
    touch "$root/allow-start"
    apply
    running
    observed secret
    snapshot > "$root/pre-reboot.snapshot"
    echo "pre-reboot smoke cases passed; evidence: $root/*.snapshot, $root/*.systemd and $root/*.journal"
    ;;
  after-reboot)
    [[ $(cat "$root/data/sentinel") == persistent-test-data ]] || fail "persistent volume lost data"
    wait_running
    observed secret
    snapshot > "$root/post-reboot.snapshot"
    apply
    snapshot > "$root/post-apply.snapshot"
    cmp -s "$root/post-reboot.snapshot" "$root/post-apply.snapshot" || fail "post-reboot apply recreated container"
    echo "all fixture smoke cases passed; evidence: $root/*.snapshot"
    ;;
  *) echo "unknown phase: $phase" >&2; exit 2 ;;
esac
