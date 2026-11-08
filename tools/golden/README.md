# Golden vectors from the real game code

Calls `Rng`, `StringHelper`, `RunRngSet`, `PlayerRngSet` and `ListExtensions` from the shipped `sts2.dll`.
Needs the .NET 9 SDK (`export DOTNET_ROOT=$HOME/.dotnet PATH=$HOME/.dotnet:$PATH`) and a game install.

    STS2_GAME="<game>/data_sts2_linuxbsd_x86_64" \
      dotnet run --project tools/golden -- crates/sts2-core/tests/golden

`STS2_GAME` defaults to the native Linux Steam library (`~/.local/share/Steam/steamapps/common`). The DLL is referenced by path, never copied.
Output is deterministic; it exits non-zero if `List<T>.Sort()` tie order disagrees with the spec's introsort.
