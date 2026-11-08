# Hits allowlist

Ported entities that no run may be expected to touch, one line per id: `- kind:Id reason`.
`tools/final_check.sh` (through `tools/hits_sweep.sh`) fails if a ported entity has zero hits and is not
listed here; `cargo xtask hits --check docs/hits-allowlist.md` reports entries that are hit after all
or no longer ported, so delete those. Every line needs a reason. `kind:Id@hive,glory` excuses the entity only in a
check whose `--acts` include one of those acts (the Hive and Glory checks in `tools/final_matrix.sh`), and is never
reported stale in another.

- relic:LastingCandy the referee has no oracle: sts2-cli never calls `BeforeCombatRewardOffered` (`NO_ORACLE_RELICS` in crates/sts2-diff/src/run_session.rs, docs/semantics/referee.md); unit-tested in the engine
- relic:WingCharm the referee has no oracle: sts2-cli answers card rewards without `CardReward.OnSelect` and the relic draws `Niche` per reward (`NO_ORACLE_RELICS`, docs/semantics/referee.md); unit-tested in the engine
- event:FakeMerchant the referee cannot enter it: sts2-cli treats an event with no `CurrentOptions` as "no options, auto-skipping" and goes straight to the map, so the run stops there (`REFEREE_SKIPPED_EVENTS` in crates/sts2-diff/src/run_session.rs, docs/semantics/referee.md); its monster and encounter are hit by regress's forced fights and its relics by the cover loadouts; the event itself is unit-tested in the engine
- monster:PaelsLegion@hive the pet `PaelsLegion` adds to every fight is a creature on the player's side that the referee's trace does not list (`enemies` holds only enemies, as for `BYRDPIP`); the relic is held in the Hive cover loadouts (`act_loadouts/cover/hive-ironclad-12.json`) and its fights match, but a trace cannot show the pet
