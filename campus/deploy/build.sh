#!/bin/sh
# Baut Browser-Teil (WASM) und Server auf dem Server selbst (2 Kerne; nie zwei cargo gleichzeitig).
# Aufruf: sh deploy/build.sh [client|server|test]   (ohne Angabe: client und server)
set -e
cd /opt/campus
B="env CARGO_BUILD_JOBS=1 nice -n 19 ionice -c 3"
W="${1:-alles}"
if [ "$W" = alles ] || [ "$W" = client ]; then
  $B cargo build --release -p campus-client --target wasm32-unknown-unknown 2>&1 | grep -E "^(error|warning: unused)|^ +-->|^ *[0-9]* *\||Finished" | head -n 60
  $B wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/campus_client.wasm
  ls -la web/pkg | tail -n 3
fi
if [ "$W" = alles ] || [ "$W" = server ]; then
  $B cargo build --release -p campus-server 2>&1 | grep -E "^(error|warning: unused)|^ +-->|^ *[0-9]* *\||Finished" | head -n 60
  mkdir -p bin && cp target/release/campus-server bin/campus-server.neu && mv bin/campus-server.neu bin/campus-server
fi
if [ "$W" = test ]; then
  $B cargo test --release -p campus-shared 2>&1 | grep -E "^test result|FAILED|panicked" | head
  $B cargo test --release -p campus-server -- --test-threads=1 2>&1 | grep -E "^test |^LAST|test result|panicked|assertion|left:|right:|^error|^ +-->" | head -n 60
fi
echo BAU-ENDE
