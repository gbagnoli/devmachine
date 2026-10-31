#!/usr/bin/env bash
# Runs only inside the disposable container started by tpm-bootstrap.sh.
set -euo pipefail
mkdir -p /run/skillet-tpm/state
swtpm socket --tpm2 --tpmstate dir=/run/skillet-tpm/state \
  --server type=tcp,bindaddr=127.0.0.1,port=2321 \
  --ctrl type=tcp,bindaddr=127.0.0.1,port=2322 \
  --flags not-need-init,startup-clear --daemon
export TPM2TOOLS_TCTI=swtpm:host=127.0.0.1,port=2321
export TCTI="$TPM2TOOLS_TCTI"
export SKILLET_SOFTWARE_TPM_ACCEPTANCE=1
status=0
/workstation-tests --ignored --exact \
  installation::tpm::runtime::tests::software_tpm_transition_and_interrupted_token_import \
  --nocapture || status=$?
# This container contains only generated fixture credentials, never a vault or
# host data. Failure stderr makes its tool acceptance errors reviewable.
if [[ "$status" != 0 ]]; then
  for diagnostic in /run/skillet-tpm-failure-*.log; do
    [[ ! -f "$diagnostic" ]] || cat "$diagnostic"
  done
fi
exit "$status"
