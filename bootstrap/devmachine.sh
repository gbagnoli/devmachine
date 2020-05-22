#!/bin/bash

set -eu
set -o pipefail

cd "${BASH_SOURCE%/*}/" || exit
# shellcheck source=include/install_chef.bash
# shellcheck disable=SC1091
if dpkg -l chef-workstation &>/dev/null; then
  ./"${BASH_SOURCE%/*}/"install_chef.sh
fi
pip3 install --user pipenv

pipenv="$(python3 -m site --user-base)"/bin/pipenv
"$pipenv" install
"$pipenv" run -- fab run
