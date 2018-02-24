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
done < <(git diff --cached --name-only --diff-filter=ACM)

if [ $total -eq 0 ];then exit 0 ; fi

bundle check >/dev/null || bundle install
git checkout-index --prefix="$tmpdir"/ -af
set +e
ec=0
if [ "${#ruby[@]}" -gt 0 ]; then
  echo "Running rubocop"
  bundle exec rubocop "${ruby[@]}"; ec=$?
fi
if [ "${#chef[@]}" -gt 0 ]; then
  echo "Running foodcritic"
  bundle exec foodcritic -B "$tmpdir/site-cookbooks" -R "$tmpdir/roles"; e=$?; [ $e -ne 0 ] && ec=$e
fi
if [ "${#python[@]}" -gt 0 ]; then
  echo "Running flake8, mypy"
  bundle exec flake8 "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  bundle exec mypy --ignore-missing-imports "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
fi
exit $ec

