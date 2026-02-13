#!/bin/bash

set -eu

pushd "$(git rev-parse --show-toplevel)" >/dev/null
tmpdir="$(mktemp -d)"

trap 'rm -rf "$tmpdir"' EXIT

ruby=()
python=()
chef=()
shell=()
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
  if [[ "$fname" == *.sh ]]; then
    shell+=("$fullpath")
  fi
done < <(git diff --cached --name-only --diff-filter=ACM)

if [ $total -eq 0 ];then exit 0 ; fi

git checkout-index --prefix="$tmpdir"/ -af
set +e
ec=0

if [ "${#ruby[@]}" -gt 0 ] || [ "${#chef[@]}" -gt 0 ]; then
  cinc exec cookstyle "$tmpdir/site-cookbooks"; e=$?; [ $e -ne 0 ] && ec=$e
fi
if [ "${#python[@]}" -gt 0 ]; then
  echo "Running ruff "
  uv run ruff check --no-fix --diff "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  uv run ruff format --diff "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  echo -n "Running mypy :"
  uv run mypy --ignore-missing-imports "${python[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
fi

if [ "${#shell[@]}" -gt 0 ]; then
  if [ -x "$(command -v shellcheck)" ]; then
    echo "running shellcheck"
    shellcheck -x "${shell[@]}"; e=$?; [ $e -ne 0 ] && ec=$e
  else
    echo >&2 "Please install shellcheck for your platform"; ec=1
  fi
fi

exit $ec
