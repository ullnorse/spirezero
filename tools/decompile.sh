#!/usr/bin/env bash
# Decompile the pinned game's sts2.dll into ref/decomp (gitignored) and rebuild ref/INDEX.tsv.
set -euo pipefail
export DOTNET_ROOT="$HOME/.dotnet" PATH="$HOME/.dotnet:$HOME/.dotnet/tools:$PATH"
GAME="${STS2_GAME:-$HOME/.local/share/Steam/steamapps/common/Slay the Spire 2}"
grep -q '"version": "v0.111.0"' "$GAME/release_info.json" || { echo "game is not v0.111.0" >&2; exit 1; }
rm -rf ref/decomp && mkdir -p ref/decomp
ilspycmd -p -o ref/decomp "$GAME/data_sts2_linuxbsd_x86_64/sts2.dll" > ref/decompile.log 2>&1
python3 tools/index_ref.py ref
