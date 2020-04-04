#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
tmpdir="$(mktemp -d)"

cleanup() {
  rm -rf "$tmpdir"
}

trap cleanup EXIT

ruby=()
python=()
chef=()
shell=()
circleci=false
total=0
while read -r fname; do
  total=$((total + 1))
  fullpath="$tmpdir/$fname"
  if [[ "$fname" == *.rb ]]; then
    ruby+=("$fullpath")
  elif [[ "$fname" == *.py ]]; then
    python+=("$fullpath")
  fi
  if [[ "$fname" == site-cookbooks/* ]] || [[ "$fname" == roles/* ]]; then
    chef+=("$fullpath")
  fi
  if [[ "$fname" == ".circleci/config.yml" ]]; then
    circleci=true
  fi
  if [[ "$fname" == *.sh ]]; then
    shell+=("$fullpath")
  fi
done < <(git diff --cached --name-only --diff-filter=ACM)

if [ $total -eq 0 ];then exit 0 ; fi

bundle check >/dev/null || bundle install
git checkout-index --prefix="$tmpdir"/ -af
set +e
ec=0

if $circleci; then
  if [ -x "$(which circleci)" ]; then
    echo "Validating CircleCI config"
    circleci config validate .circleci/config.yml ; ec=$?
  else
    echo "Cannot validate CircleCI config, missing CLI"
  fi
fi
if [ "${#ruby[@]}" -gt 0 ]; then
  echo "Running rubocop"
  bundle exec rubocop --config "$tmpdir/.rubocop.yml" "${ruby[@]}"; ec=$?
fi
if [ "${#chef[@]}" -gt 0 ]; then
  echo "Running foodcritic"
  bundle exec foodcritic -B "$tmpdir/site-cookbooks" -R "$tmpdir/roles"; e=$?; [ $e -ne 0 ] && ec=$e
fi
if [ "${#python[@]}" -gt 0 ]; then
  echo "Running black "
  pipenv run black --check --diff "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  echo "Running isort "
  pipenv run isort "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  echo "Running flake8 "
  pipenv run flake8 --ignore=E501 "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  echo "Running mypy "
  pipenv run mypy --ignore-missing-imports "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
fi

if [ "${#shell[@]}" -gt 0 ]; then
  if [ -x "$(which shellcheck)" ]; then
    echo "running shellcheck"
    shellcheck "${shell[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  else
    echo >&2 "Please install shellcheck for your platform"; ec=1
  fi
fi

exit $ec

