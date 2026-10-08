#!/bin/sh
set -eu
stdb_test_root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$stdb_test_root"
GOROOT=$(go env GOROOT)
export GOROOT
exec node "$stdb_test_root/tools/server/wasm-account-test.cjs" "$@"
