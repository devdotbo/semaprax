#!/bin/sh
set -eu
app_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ ! -x "$app_dir/loglens" ]; then "$app_dir/build.sh" >/dev/null; fi
if [ "$#" -eq 0 ]; then exec "$app_dir/loglens"; fi
exec "$app_dir/loglens" --spx-selected-file-v1 0 -- "$@"
