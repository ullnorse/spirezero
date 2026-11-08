#!/bin/sh
# Builds the wasm modules: the page's into www/pkg and the bot's (sts2-web-bot, loaded by the Web
# Worker on demand) into www/pkg-bot. Needs `rustup target add wasm32-unknown-unknown` and
# `cargo install wasm-bindgen-cli --version 0.2.129 --locked` (the version of the wasm-bindgen crate).
# `wasm-opt` (binaryen), when on PATH, shrinks them further.
# WWW=<dir> writes into <dir> instead of www/ (deploy.sh builds releases that way).
set -eu
WWW=${WWW:+$(realpath "$WWW")}
cd "$(dirname "$0")/../.."
cargo build -p sts2-web -p sts2-web-bot --profile web --target wasm32-unknown-unknown
for crate in sts2_web:pkg sts2_web_bot:pkg-bot; do
  name=${crate%%:*}
  out=${WWW:-crates/sts2-web/www}/${crate##*:}
  wasm-bindgen --target web --no-typescript --out-dir "$out" "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/web/$name.wasm"
  if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
      -o "$out/${name}_bg.wasm" "$out/${name}_bg.wasm"
  fi
  ls -l "$out/${name}_bg.wasm"
done
