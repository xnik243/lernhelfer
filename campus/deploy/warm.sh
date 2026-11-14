#!/bin/sh
# Abhängigkeiten vorbauen (ein Kern, niedrige Priorität).
cd /opt/campus || exit 1
B="env CARGO_BUILD_JOBS=1 nice -n 19 ionice -c 3"
$B cargo build --release -p campus-server 2>&1 | tail -n 15
$B cargo build --release -p campus-client --target wasm32-unknown-unknown 2>&1 | tail -n 15
command -v wasm-bindgen >/dev/null || $B cargo install wasm-bindgen-cli --version 0.2.100 --locked --root /usr/local 2>&1 | tail -n 5
echo WARM-FERTIG
