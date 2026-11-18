#!/bin/bash
# Rebuilds the FFI library without touching the file running jobs have mapped: the old one is
# unlinked first (a mapped inode stays valid), then cargo writes a new one.
set -e
cd "$(dirname "$0")/../.."
rm -f target/ffi/libsts2_ffi.so target/ffi/deps/libsts2_ffi.so
cargo build --profile ffi -p sts2-ffi 2>&1 | tail -1
