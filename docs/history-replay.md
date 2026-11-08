# History replay (H5, H6)

`tools/replay_history.py` checks the engine against recorded `.run` files: the user's history dirs, or (`--db`) the community dump at `~/.cache/spirezero-data/sts2fun/sts2_public.db`. The dump is third-party data with no license: never commit runs or extracts of it. This page holds counts only, plus seeds for repros.

## Sources (searched 2026-10-23; re-run `--db` to refresh)
| source | what | runs | v0.111.0 |
|---|---|---|---|
| sts2.fun public dump (`https://sts2.fun/static/sts2_public.db.zst`, linked from its FAQ "Can I download the data?"; 760 MB zstd SQLite, 13 GB unpacked; Python 3.14 reads it with `compression.zstd`) | anonymised community runs, `runs.raw_json` is the `.run` file | 472,593 (builds v0.98 to v0.111.0) | 9,588 (2.0%); 8,126 are solo, no modifiers, valid, with a seed; 8,046 after dedupe on (seed, start time) and dropping runs abandoned before Neow |
| the user's own history (`~/.local/share/SlayTheSpire2/steam/*/{,modded/}profile*/saves/history`, read-only; counts below are from the old Windows copy) | 397 files (171 profile1, 226 modded/profile1; profile2 and 3 are empty) | 397 | 379 files; 216 usable (the rest are co-op, modified or abandoned) |

Looked at and not usable: spire-codex.com (runs auto-upload per Steam login; no bulk export or published dump), ststracker.app (657,226 runs, site only, upload/login, no download), SpireMeta.gg and MetaBot.GG (aggregates; uploads feed a private pool), Untapped.gg (statistics only), and no GitHub, Kaggle, Hugging Face or Zenodo dataset of STS2 `.run` files turned up. Only the sts2.fun dump is a freely published copy; it is the same data H5 used (8,126 v0.111.0 solo runs), so a second independent corpus does not exist yet. Revisit if one appears.

## Method
- Runs: build v0.111.0, standard mode, no modifiers, one player, a seed. All 5 characters, both Act 1s, mostly A10.
- `sts2-diff history-predict --batch` builds the run for the file's character, ascension and recorded Act 1. The map walk (`crates/sts2-diff/src/history.rs`, Acts 1-3) reports the first mismatch per act. The reward replay (`history_replay.rs`) runs Neow on the engine's `Run`, taking what the player took. Each later floor then runs the engine's generators on a `RunState` fed the recorded gold, HP, deck, belt and relics; the relic bags lose every relic a floor offered.
- No fight is played (the file has no combat log). The `rewards` stream chains across floors. Some consumers aren't modelled: events, the Act 2 and 3 ancients, rest heals with a reward hook, and gold that a fight adds (`HeistPower`, `RoyaltiesPower`). After one, the next fight or shop is matched by searching 0-400 draws ahead (a resync). Checks after a resync are counted apart and are weak evidence: the search picks whichever draw count reproduces the card and potion sets.
- H6 additions: the replay now covers all three acts (events, chest relics and, after resyncs, rewards and shops). Golden Compass is walked (golden-path map in its act, `?` rooms always events). A deck `LanternKey` is walked (Glory's `?` rooms are events; `WAR_HISTORIAN_REPY` replaces the pulled event but the list still advances). Each act's ancient advances the act's event cursor (`MarkRoomVisited`). `TotalFloor` includes the current floor for rewards and chests (`IsBeforeAct3TreasureChest`, which gates Shovel, Lucky Fysh, Juzu Bracelet, the eggs and Book of Five Rings) and excludes it for the event pull (`CreateRoom` runs before `AppendToMapPointHistory`).
- PX19 additions: event relic pulls the file only names by title (`hidden_relic_pulls`: Relic Trader, Wongo's featured relic), a skipped chest's relic leaves the shared bag, melted Toy Box wax relics stop being held (`melt_floors`), a first-run `?` re-walk (`first_run_fits`), the Kaleidoscope epoch rule and `CLIENT_CASES`.
- PX25 (redo of the lost px24-d): a shop lists the potion it sold in `potion_choices` (picked) as well as `bought_potions`, so the tracked belt takes the multiset union of the two. Counting both left `MFNTU0QSTUTY` (Regent, A10) with a phantom `CURE_ALL` after f33, and its f37 event pull (`IsAllowed` of Ranwid, Stone of All Time and The Future of Potions read the belt) predicted `TRIAL` for the recorded `CRYSTAL_SPHERE`. Test: `history_replay::tests::shop_bought_potion_counts_once`.
- Runs: `--db` replays all runs in about 5 min with 12 jobs (`STS2_REF=$PWD/ref`). `tools/final_check.sh` runs `--sample 300` per character × Act 1 and skips this step without the DB. `--reuse` reclassifies cached predictions. `--run SEED` replays one run and prints every check and gap (a repro). Derived files go to `~/.cache/spirezero-data/h5/` and `target/`.

## What is checked
- Act 1 roll; per act (1-3): map, `?` rooms, encounters, boss, ancient id, event order. Per `?` event: the exact event (all acts). Neow's options, relics gained, and the cards and potions its prompts offer. Rewards in every act: fight cards (with upgrades), potion, elite relic and gold; chest relic and gold; shop cards, relics and potions; Dig relic. Acts 2 and 3 rewards are only reachable after a resync (the ancient and events break the chain), so they are in the "after a resync" count, not the tables.
- Not checkable from the file: anything inside a fight (HP, potions used, generated cards, `GoldProportion`); shop prices; card order within an offer (compared as sets); event rewards; Act 2-3 ancient options (not ported).

## Pass rates, community dump (match / checked, %; profile-state mismatches excluded, after-resync checks not included; PX19 rerun on `main` after C25, NR1 and SP1)
| check | Iro O | Iro U | Sil O | Sil U | Def O | Def U | Reg O | Reg U | Nec O | Nec U | all |
|---|---|---|---|---|---|---|---|---|---|---|---|
| act1 | 99.9 | 100.0 | 99.9 | 99.8 | 99.9 | 99.7 | 99.7 | 100.0 | 100.0 | 99.8 | 8031/8041 |
| map (Acts 1-3) | 98.7 | 99.1 | 97.2 | 97.3 | 97.9 | 98.2 | 97.6 | 99.1 | 99.3 | 99.2 | 16632/16927 |
| `?` rooms | 99.0 | 99.4 | 98.9 | 98.9 | 99.4 | 99.7 | 99.6 | 99.9 | 99.8 | 99.8 | 16141/16243 |
| encounters | 100.0 | 99.9 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 99.8 | 99.9 | 16141/16146 |
| boss | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 16141/16141 |
| ancient | 99.9 | 99.8 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 99.9 | 100.0 | 100.0 | 16141/16146 |
| event order | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 16141/16141 |
| event (exact) | 99.4 | 99.4 | 99.4 | 99.4 | 99.2 | 99.4 | 99.1 | 99.7 | 99.6 | 99.7 | 34222/34418 |
| Neow options | 99.6 | 100.0 | 99.8 | 99.8 | 99.6 | 99.7 | 99.6 | 99.9 | 100.0 | 99.7 | 8008/8026 |
| Neow offers, relics | 99.9 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 15477/15478 |
| card reward (Act 1) | 98.9 | 98.8 | 99.1 | 99.0 | 98.9 | 98.9 | 98.5 | 98.8 | 98.5 | 99.1 | 18565/18776 |
| potion reward (Act 1) | 99.3 | 99.2 | 99.1 | 99.3 | 99.1 | 99.5 | 99.1 | 99.4 | 99.2 | 99.3 | 18737/18880 |
| elite relic (Act 1) | 92.9 | 92.2 | 93.8 | 88.7 | 93.8 | 91.6 | 95.0 | 94.0 | 78.2 | 91.7 | 965/1053 |
| fight gold (Act 1) | 98.0 | 97.7 | 99.0 | 98.6 | 98.2 | 98.9 | 98.0 | 98.3 | 98.4 | 98.1 | 18542/18853 |
| chest relic (Acts 1-3) | 99.2 | 99.5 | 99.5 | 99.6 | 99.1 | 99.6 | 98.9 | 99.4 | 99.9 | 99.5 | 16692/16787 |
| chest gold | 100.0 | 100.0 | 100.0 | 100.0 | 100.0 | 98.3 | 100.0 | 100.0 | 100.0 | 100.0 | 660/661 |
| shop cards (Act 1) | 99.0 | 99.5 | 98.5 | 99.3 | 97.6 | 99.4 | 98.8 | 98.8 | 98.8 | 98.4 | 1874/1896 |
| shop relics, potions (Act 1) | 100.0 | 100.0 | 100.0 | 100.0 | 99.4 | 100.0 | 100.0 | 99.7 | 100.0 | 99.2 | 3788/3793 |

The map rows count every mismatch of the walk, including the classes below (Winged Boots, profile state), so they read lower than the engine's real rate.

Per act (the same counts split by the act of the floor):
| check | Act 1 | Act 2 | Act 3 |
|---|---|---|---|
| map | 7860/8046 | 5334/5405 | 3438/3476 |
| `?` rooms | 7700/7701 | 5152/5203 | 3289/3339 |
| encounters, boss, ancient, event order | 7700/7705, 7700/7700, 7700/7700, 7700/7700 | 5152/5152, 5152/5152, 5152/5155, 5152/5152 | 3289/3289, 3289/3289, 3289/3291, 3289/3289 |
| event (exact) | 16644/16699 | 10787/10863 | 6791/6856 |
| chest relic | 7720/7754 | 5366/5404 | 3606/3629 |

After a resync, all acts: 269,081/288,728 match (93.2%; the same binary with the pre-PX19 tool: 267,564/287,654). By act the fight-card check is 94% (Act 1), 91% (Act 2), 96% (Act 3). That figure is a lower bound on the engine: the resync search, unmodelled event rewards and the weaker Act 2 and 3 chain (ancients, Act 2+ events) all show up as misses here. They are classed `file`, not bugs.

## Pass rates, the user's history (216 runs)
4,836/4,875 checks match (PX19 rerun); 15 excluded as profile state; 0 unexplained. By act: map 338/339, `?` rooms 333/333, encounters, boss, ancient, event order 333/333 each, events 565/566 (the miss needs Kaleidoscope), chest relics 261/261 (Acts 1-3), Neow options 198/216 (the 18 misses are older saves with no `ancient_choice`).

## Mismatch classes (community dump, every check, after-resync ones included)
- **Profile, 4,434, excluded automatically.** Epoch rule (a run is unlock-proven if, for every epoch its pools can draw from, some run by the same player at or before it shows that epoch's content), character rule (Kaleidoscope needs every character: `Silent1`, `Regent1`, `Necrobinder1`, `Defect1` epochs), `PROFILE_CASES` by seed (two Colorless2Epoch shops), boss discovery order, Underdocks locked. Validation: 1,002 runs are unproven; only they show most pool-dependent mismatches.
- **File format, 19,940:** after a resync (the bulk); run ended on that floor; `gold_gained` counts other gold; uploader's `old`-prefixed seeds; potions procured in the fight; offers from a relic bought on the floor (Orrery, Courier); chest relic not taken; egg upgrades on the same screen; events whose `IsAllowed` reads the deck, belt, relics, HP or gold (Amalgamator, Ranwid, Grave of the Forgotten, Colossal Flower, Relic Trader, Fake Merchant: rebuilt approximately).
- **Player:** Winged Boots free travel (map, `?`), modded clients, and one uploader's client (`CLIENT_CASES`, below).
- **Engine gaps, 1,455:** an unported held relic (Kaleidoscope above all) and Neow's Bones stopping at an unported gift. Golden Compass was 98 map misses in H5 and is now walked (0).
- **Unexplained: 0** (PX19; H6 left 69 checks in 42 runs, resolved below). The user's own history has none either.

## Engine-bug candidates (H6's 69, resolved by PX19)
None is an engine bug. Per class (H6 counts; repro `STS2_REF=$PWD/ref python3 -I tools/replay_history.py --db --run SEED`):
1. **Chest relic, 44 checks in 34 runs: replay-tool bugs (23 runs) and one client (11 runs).** In every non-client case the file's relic is the one right after the engine's in the shared bag's bucket, so the game had lost one relic the file never names. Fixed in the tool:
   - `RelicTrader` (19 runs) pulls three `NewRelics` with `PullNextRelicFromFront`, which take them out of both bags; the file records only the traded one in `relic_choices`. The other two are only in the chosen option's `variables` (`TopRelicNew`, `MiddleRelicNew`, `BottomRelicNew`), as localised titles. `WelcomeToWongos` (2 runs) pulls its `FeaturedItem` when it opens, bought or not (`RandomRelic`). The tool maps titles to ids (English by spelling; other languages from the dump's own traded-option/relic pairs, 1,175 titles, cached in `~/.cache/spirezero-data/h5/relic-titles.json`; 99.7% of offered titles map) and passes them as `hidden_relic_pulls`, which `history_replay.rs` takes out of the bags. If a title doesn't map, later relic checks are classed `file`.
   - A chest whose relic was left (2 runs, `QCJ30V7GN43L`, `2G61871HUARV`) had already pulled it at `TreasureRoom.EnterInternal` → `BeginRelicPicking`; the replay put it back. Now the shared bag loses it.
   - The tool's `CPEK8RBPWVDB` f26 + f41 pairs and similar are one lost relic, counted at each later chest of that rarity.
2. **`?` room, 23 checks in 15 runs.** 4 runs (`FD0ENXW5NYRH`, `HEDQ6ZUBKC8F`, `YH8K8DTMHHDN`, `FC0T4Y06K9XR`) were a replay-tool bug: a `ToyBox` wax `JuzuBracelet` that had melted still removed Monster. `ToyBox.AfterCombatEnd` melts the first unmelted wax relic every 3rd combat, and `RunState.IterateHookListeners` skips `IsMelted` relics. The file has `IsWax`/`IsMelted` but no floor, so `melt_floors` counts the combat rooms. 1 run (`ZHZHRGSTRVEH`) is a profile's first run (`NumberOfRuns == 0`, at A10): `UnknownMapPointOdds.Roll` makes the first two `?` events and the third a monster with no draw (every later `?` fits the stream 3 draws back), and `Overgrowth.ApplyActDiscoveryOrderModifications` fixes the Act 1 order (Nibbits Weak first, Sapphire Seed first event). `Run::new` rejects first-run states, so `history-predict` takes a `first_run` flag for the walk only. The tool re-walks every run with a `?` mismatch that way, and a run whose room types then all match is classed `profile`. The other 10 runs are the client below.
3. **Event, 2 checks** (`U0JD0F368UHZ`, `62383M4TXRJ2`, Act 2 f23): they match with the engine on `main` (merges after H6's predictions, C25/NR1/SP1), with the old tool too.
4. **Client artifact: uploader 10001215 (13 runs, 38 `?` and chest checks; `CLIENT_CASES`, class `player`).** That is H6's 12 plus `V7MRTMM5LAQ3`, whose held relic is now ported. 13 of their 30 v0.111.0 runs miss. The other 8,016 runs had 30 such runs under H6 and have none after PX19's fixes. Two independent streams disagree in the same runs: `?` rolls (`HULYAJEMURRQ` matches through f11, then its f13 `?` is a treasure where the next `UnknownMapPoint` draw is 0.806, an event at any decomp odds; no offset within ±4 draws gives `SGHSU9TF1NAE` f37 its treasure) and chest picks (`SGHSU9TF1NAE` skips `ORICHALCUM` at f26 and f37, then gets it at f41, so the bag's order or the rarity stream differs, not just one lost relic). Nothing in the files marks the client as modded. Their profile also went from A10 back to A4 on v0.110.1.

Also from PX19's rerun: 6 Kaleidoscope Neow picks (ported by SP1 after H6) miss because `Kaleidoscope` draws from the other characters' pools, whose epochs `relevant_epochs` ignored. They are now `profile` unless every epoch is proven.

Fixed in the tool by H6 (they looked like engine bugs): Act 3 chest relics (454 misses at f41 were the tool's `TotalFloor` one short, so Shovel, Lucky Fysh, Book of Five Rings and the like were wrongly allowed at the Act 3 chest; 27 remain); Act 2 and 3 events (39 of 224 missed in the user's history until the ancient advanced the event cursor, and 68 in the dump until the Lantern Key swap was handled); 63 `?` misses (Golden Compass and Lantern Key not walked; 23 remain).

H5's earlier verdicts still hold: shop colorless slot 2 (profile, Colorless2Epoch), Neow options 3 (character rule), Neow's Bones → Large Capsule (reward order), potion reward 1 (died in the fight), the Silent U `R7PUVGVBNCET` chest (tool). The Ironclad U `HULYAJEMURRQ` A7 case is closed by PX19: its uploader's client (item 4 above).
