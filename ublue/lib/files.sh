#!/bin/bash

copy() {
   # return 0 if file is copies, or 1 if not
   # usage: copy <src> <dst> [ any extra args to install ]
   # always use sudo - so if you need a specific user, pass it to install
   # (-u <user> -g <group>)
   local src="$1"
   local dst="$2"
   shift 2
   old_time=$(stat -c %Y "$dst" 2>/dev/null)
   sudo install -C -v "$@" "$src" "$dst"
   new_time=$(stat -c %Y "$dst" 2>/dev/null)
   [ "$old_time" != "$new_time" ]
}
