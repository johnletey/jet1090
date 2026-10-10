#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
buf generate
buf build --as-file-descriptor-set --exclude-source-info \
  -o crates/jet1090/src/generated/snapshot.fds.bin
