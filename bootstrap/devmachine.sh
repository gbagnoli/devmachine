#!/bin/bash

set -eu
set -o pipefail

cd "${BASH_SOURCE%/*}/" || exit
# shellcheck source=include/install_chef.bash
# shellcheck disable=SC1091
source "${BASH_SOURCE%/*}/"include/install_chef.bash

pip3 install --user pipenv

pipenv="$(python3 -m site --user-base)"/bin/pipenv
"$pipenv" install
"$pipenv" run -- fab run
