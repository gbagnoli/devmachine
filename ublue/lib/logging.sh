#!/bin/bash

BB="\e[1m"
BE="\e[0m"

disable_colors() {
  BB=""
  BE=""
}

if [ ! -t 1 ] || [ "$(tput colors 2>/dev/null || echo 0)" -lt 8 ]; then
  disable_colors
fi

log() {
  echo -e >&2 "$BB* $*$BE"
}
