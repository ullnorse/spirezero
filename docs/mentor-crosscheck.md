# Cross-check against spire-mentor (XM1)

spire-mentor (`sts2-rs`, MIT) is a second, independent Rust port with its own parity record. It was built for stable v0.107.1 and has a
v0.111.0 overlay (`sts2-content/src/cards/v0111.rs`, `GameVersion::V0_111_0`, each row citing the v0.111.0 decomp) for part of the
content. This note records what was compared with SpireZero (v0.111.0), every disagreement, and who is right. The decomp in
`ref/decomp/` (v0.111.0) is the judge.

**Result: no SpireZero bug found.** Every disagreement at v0.111.0 is a representation difference, a mentor error, a mentor
simplification, or content the game no longer has. Verdicts below. Mentor's v0.111.0 overlay agrees with SpireZero on every card
spec value, monster HP and attack value it overrides.

Reproduce: `tools/mentor_crosscheck/run.sh` (read-only on spire-mentor: its `sts2-content` is a path dependency built into
`target/xm1/mentor_target`; set `MENTOR` for another checkout). It dumps both ports' data as TSV under `target/xm1/` and prints the
differences; the Rust dumpers and Python comparers are in `tools/mentor_crosscheck/`.

## What was compared

| kind | how | entities | compared fields |
|---|---|---|---|
| card | mentor `CardSpec` (`spec_for`, `upgrade_delta_for`) at v0.107.1 and v0.111.0 vs `CardKind` + `RunCard::upgrade` | 558 SpireZero / 598 mentor rows | type, target, rarity, cost (X cost skipped), keywords, base damage and block (matched against SpireZero's `Damage`, `OstyDamage`, `CalculationBase`, `Block` ... vars), and the upgraded cost, keywords, damage and block where mentor has a spec-level upgrade |
| monster HP | `starting_hp_unique_for` (300 rolls) vs spawned `max_hp` of every encounter, A0/A8/A9 | 103 monsters | min and max HP |
| monster attacks | mentor `build_ai_for_version` move intents vs SpireZero `intent_damage` of every move state forced on a scratch copy of each encounter's combat (and observed in play for 120 seeds x 40 turns), A0/A8/A9 | 103 monsters | damage x hits per move (matched by move id and, because the two ports name moves differently, as a set), minus Strength |
| monster move graph | mentor `MonsterAi` states vs SpireZero `MoveTable` | 91 monsters in both | initial state, move follow-ups, random-branch targets, weights, repeat type, `maxTimes`, cooldown, conditional targets |
| encounter | mentor `EncounterDef.monsters` vs the spawned roster (60 seeds) | 81 mentor / 87 SpireZero | monster multiset |
| relic | integer constants of each relic file, matched by name | 182 relics with constants in both | value sets |
| potion | integer literals per file | 63 | value sets (heuristic, see below) |
| ascension (run) | A2 heal, A3 gold, A6 removal price | 3 | values |
| not compared | event option numbers, power amounts and hook timing (no per-entity tables on either side), block/buff/debuff amounts inside monster moves (only attack damage and hits are compared) | | |

## Disagreements and verdicts

### Cards (15 of 558 differ at v0.111.0; 40 mentor-only)

| entity | SpireZero | mentor | verdict |
|---|---|---|---|
| `BURN`, `DECAY`, `INFECTION`, `TOXIC` | `DamageVar` 2, 2, 3, 5 (their `OnTurnEndInHand` hit) | `base_damage` 0 (effect handler) | representation; SpireZero matches the decomp (`Burn.cs: new DamageVar(2m, Unpowered|Move)`) |
| `MAD_SCIENCE`, `SECOND_WIND` | `BlockVar` 8 / 5 (+2 upgraded) | `base_block` 0 | representation; decomp `SecondWind.cs: BlockVar(5m)`, `MadScience.cs: BlockVar(8m)` |
| `BYRDONIS_EGG`, `DOWSING`, `LANTERN_KEY`, `SPOILS_MAP` | `CardType::Quest` | `Skill` | SpireZero right (`CardType.Quest`, `base(-1, CardType.Quest, CardRarity.Quest, ..)`) |
| `DISINTEGRATION`, `MIND_ROT`, `SLOTH`, `WASTE_AWAY` | no `Unplayable` keyword | `Unplayable` | SpireZero right: no keyword in the decomp class (they are KnowledgeDemon choices / statuses whose power does the work); mentor models "cannot be played" as a keyword |
| `FOLLY` | Unplayable, Eternal, Innate, **Ethereal** | Unplayable, Eternal, Innate | SpireZero right (`Folly.cs` lists four keywords, Ethereal last). Mentor lacks Ethereal, so its Folly would not exhaust at turn end |

Mentor-only cards (40): 37 are `MultiplayerOnly` in the decomp (SpireZero is single-player; `docs/coverage-exclusions.md`), and `PAIN`,
`PARASITE`, `SCARE` no longer exist in v0.111.0 (no class in `ref/`; `Scare` was replaced by `Sidestep` in the Silent pool, as mentor's
`pool_substitute` also says). Mentor still lists Pain and Parasite for v0.111.0.

v0.111.0 changes in mentor's overlay (43 cards: rarity, cost, damage, block, keyword, upgrade deltas, e.g. `Mangle` 15 -> 20,
`Devastate` 30 -> 35, `Rend` cost 2 -> 1, `Taunt` 7 -> 6 and Common, `Outbreak` Power -> Skill, `Haze` loses Sly, `Whistle` 3 -> 2): SpireZero
agrees on every spec field. The "behaviour rows" mentor marks as its S8a2 (`Hyperbeam`'s focus-down power, `Eidolon` auto-play,
`ExpectAFight`, `Outbreak`) are outside this comparison.

### Monsters

HP: 0 differences against mentor's v0.111.0 for 103 monsters at A0, A8 and A9 (mentor's v0.111.0 overrides Exoskeleton 26-30 and
Entomancer 165 at A8, both matched by SpireZero).

Attack values: one real disagreement.

| monster | SpireZero | mentor | verdict |
|---|---|---|---|
| `GremlinMerc` at **A8** (`GIMME` 8x2, `DOUBLE_SMASH` 7x2, `HEHE` 9) | scaled from ToughEnemies (A8) | scaled from DeadlyEnemies (A9): 7x2, 6x2, 8 at A8 | SpireZero right for v0.111.0: `GremlinMerc.cs` uses `GetValueIfAscension(ToughEnemies, 8, 7)`, `(.., 7, 6)`, `(.., 9, 8)`. Mentor puts all three in its A9 table (same at v0.107.1 in mentor; whether the game changed between builds is unknown) |

Against mentor's v0.107.1 build the same comparison reports the v0.111.0 changes SpireZero has already ported (Aeonglass `EBB` 26 -> 22,
Axebot 12/9 -> 14/10, MechaKnight's attacking Flamethrower, SoulFysh `DE_GAS` A9 17 -> 18, TorchHeadAmalgam, Exoskeleton and Entomancer
HP), a useful sign that the diff is sensitive.

Not disagreements, filtered in the comparer: SpireZero's intent includes Strength (subtracted), `Dexterity` for TheForgotten's Dread
(15 at A9, decomp `+ Creature.GetPowerAmount<DexterityPower>()`), and the +50% of `Surrounded`/`BackAttack` on Kaiser Crab, Torchhead and
Queen fights.

Move graphs: 51 of the 91 monsters in both ports have identical graphs (including weights, repeat types and cooldowns). The other 40
differ only by names (`MAGIC_BOMB` / `BOMB`, `INIT` states for conditional openers, `THRASH_MOVE_2` / `THRASH_2`) or by mentor
folding the spawn slot into the opener (Nibbit, Inklet, Wriggler, TwoTailedRat's `RAND` as two conditionals, CorpseSlug, ScrollOfBiting)
where SpireZero keeps the decomp's machine and branches on `SetUpForCombat` fields. Checked against the decomp: Queen's conditional order
(irrelevant, the conditions are exclusive), BygoneEffigy `SLEEP -> WAKE -> SLASHES`, SlumberingBeetle `SNORE -> SNORE_NEXT`,
TwoTailedRat's dynamic weights (1 or 1/12, and 0.75 for the summon) are as SpireZero has them.

### Encounters

80 of 81 mentor encounters spawn the same monsters in SpireZero. `TunnelerNormal` is the exception: the class exists, but nothing in
`ref/` references it (not in any act's pool or event), so SpireZero rightly omits it. SpireZero's 7 extra are event encounters
(`BattlewornDummy*`, `DenseVegetationEvent`, `FakeMerchantEvent`, `MysteriousKnightEvent`, `PunchOffEvent`).

### Relics, potions, ascension

Relic constants: 23 of 182 differ as value sets. Four are v0.111.0 changes mentor carries under `*_V0111` names that SpireZero already
has (`BeautifulBracelet` 4/2, `DistinguishedCape` 2 curses, `Regalite` 4, `SereTalon` 9 HP); the rest are naming (`BoomingConch`,
`BlessedAntler`, `Nunchaku`) or mentor not using a constant. The ones that looked like value differences were checked in the decomp and
SpireZero is right: `FurCoat` 8 combats, `SignetRing` 888, `SealOfGold` 3 gold, `Orrery` 5 cards, `GlassEye` 5 rewards of 3,
`JossPaper` 5/1. Potions: `BoneBrew` 15, `KingsCourage` 15, `CosmicConcoction` 3, `PotOfGhouls` 2 are as in the decomp; no unexplained
difference. The integer-set comparison is blunt (it cannot see a wrong use of the right constant).

Run-level ascension values (A2 `WearyTraveler` x0.8, A3 `Poverty` x0.75, A6 `Inflation` 100 + 50n / 75 + 25n) agree.

## Targeted diffs against the referee

No disagreement needed a referee verdict (the decomp settled all), but mentor's parity history lists mechanics that tripped it. Each
was checked for SpireZero coverage; those without a loadout got one (all pass the referee, 5 seeds x A0 and A10, 8 seeds for runs):

| mentor trouble (its parity sweep) | SpireZero coverage before | new loadout |
|---|---|---|
| SicEm applied past `Artifact` (seeds 136/306), Artifact-warded Chomper, Doom onto an Artifact stack (seed 492), Cubex Artifact | `p9-sic_em` etc. on Cubex only | `xm1-sic_em_artifact`, `xm1-doom_artifact`, `xm1-potion_doom_artifact`, `xm1-ironclad_debuffs_artifact`, `xm1-silent_debuffs_artifact` on every Artifact monster (Chompers, Cubex, PunchConstruct, MechaKnight, Aeonglass) |
| SharedFate onto a +2 Strength enemy left an inert 0-amount power (seeds 139/422, Regent 868 / TurretOperator, 353) | `p9-shared_fate` on 5 starter fights | `xm1-shared_fate_strength`, `xm1-silent_strength_zero` (Piercing Wail, Malaise, Dark Shackles) on Nibbits, FuzzyWurm, TurretOperator, Cultists, Exoskeletons |
| Flutter 5 -> 4 decrement | no ThievingHopper loadout with multi-hit | `xm1-silent_shivs_flutter` (Shivs, Dagger Spray) on ThievingHopperWeak and others |
| PillarOfCreation block, generated cards entering piles with an empty Play pile (Regent 104/400/427/438/495, Defect 111), CalculatedGamble | `pr5-pillar_of_creation`, `ps2-calculated_gamble` | `xm1-regent_generators`, `xm1-defect_generators` |
| `NEOWS_TALISMAN` upgrade must apply the stat delta, not only the flag | none | `run_loadouts/xm1-talisman_boots_holster` (with `WINGED_BOOTS`, `PHIAL_HOLSTER`) |
| other Neow/ancient relics mentor lists as late ports (`SILKEN_TRESS`, `FISHING_ROD`, `KALEIDOSCOPE`, `HEFTY_TABLET`) | none | `xm1-silken_tress_fishing_rod`, `xm1-kaleidoscope`, `xm1-hefty_tablet` |
| `NUTRITIOUS_SOUP` makes Strikes Eternal (removal filter), `SELF_FORMING_CLAY` | `pi4-self_forming_clay` (combat only) | `xm1-nutritious_soup` (whole runs) |

Already covered before this task (loadout names): Death's Door `p6-deaths_door`, Unmovable `c20-unmovable`, Nimble `c13-nimble`,
Catastrophe `p12-catastrophe`, MadScience via `c24-tinker_time_mad_science`, `NEOWS_BONES`.

Most of the rest of mentor's anecdotes concern its own oracle harness (shop cost jitter, auto-collected rewards, the event seed's
`NetId`) or run-layer plumbing, not engine rules; none was turned into a loadout.

## Ascension sweep (A5, A8, A9)

`tools/regress.sh` runs ascension 0 and 10 only. Every `ToughEnemies` (A8) and `DeadlyEnemies` (A9) threshold is therefore
indistinguishable from the other at A10, and the one value mentor and SpireZero disagree on (GremlinMerc at A8) sits exactly there. The
sweep below plays every encounter on the referee at A5, A8 and A9.

Result: 87 encounters x 3 ascensions x 6 seeds (Necrobinder, random policy, `tools/diff.sh`), 261 of 261 runs match. So SpireZero's A8 values
(GremlinMerc included) are confirmed by the referee, not only by reading the decomp. Not added to `tools/regress.sh` (it would triple its
bare-encounter runs); `ASCENSIONS="5 8 9" tools/regress.sh` repeats it with the loadouts.

## Confirmed SpireZero bugs

None. (`tasks/BACKLOG.md` unchanged.)

## Mentor errors found (for information)

- `Folly` has no Ethereal; `Pain`, `Parasite` remain in the v0.111.0 build; Quest cards are Skills; the status cards above carry Unplayable.
- `GremlinMerc` scales at A9 instead of A8.
- `Exoskeleton` and `Entomancer` HP, and the attacks listed above, are right only in mentor's v0.111.0 mode.
