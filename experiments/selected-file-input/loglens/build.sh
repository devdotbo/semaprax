#!/bin/sh
set -eu
cd "$(dirname "$0")"
compiler=${SEMAPRAX_BIN:?set SEMAPRAX_BIN to the qualified current compiler}
"$compiler" check semaprax.toml
rm -f loglens
"$compiler" build --manifest-path semaprax.toml --target native --output loglens
