# Characters survey (S-CHAR)

Read-only survey for C15 (the `Character` enum and a superset `Player`). Decomp paths are relative to `ref/decomp/`. Short names: `Chars/` = `MegaCrit.Sts2.Core.Models.Characters/`, `Pools/` = `MegaCrit.Sts2.Core.Models.{Card,Relic,Potion}Pools/`, `Epochs/` = `MegaCrit.Sts2.Core.Timeline.Epochs/`, `Players/` = `MegaCrit.Sts2.Core.Entities.Players/`. Card, relic and potion counts were scripted from `ref/INDEX.tsv` plus the first `CardRarity`/`CardType`/`RelicRarity`/`PotionRarity` in each class.

## 1. `CharacterModel` (`MegaCrit.Sts2.Core.Models/CharacterModel.cs`)
The abstract members (`:93-113`) are `StartingHp`, `StartingGold`, `CardPool`, `RelicPool`, `PotionPool`, `StartingDeck` and `StartingRelics`. The virtual members with defaults:
- `MaxEnergy` = 3 (`:97`).
- `BaseOrbSlotCount` = 0 (`:101`).
- `ShouldAlwaysShowStarCounter` = false (`:103`).
- `StartingPotions` = empty (`:115`).
- `IsPlayable` = true (`:35`).
- `UnlocksAfterRunAs` (`:63`), a progression gate that doesn't affect combat.

Everything else is art, audio or animation. Potion slots and hand size are **not** character fields. Slots come from `Player.initialMaxPotionSlotCount = 3` (`Players/Player.cs:27`, passed at `:307`), and `TightBelt` takes one. The first-turn draw is `Hook.ModifyHandDraw(..., 5m, ...)` (`Combat/CombatManager.cs:902`) and the cap is `CardPile.MaxCardsInHand = 10` (`Entities.Cards/CardPile.cs:21`). `Player.CreateForNewRun` (`:307`) builds `Player(character, netId, StartingHp, StartingHp, MaxEnergy, StartingGold, 3, BaseOrbSlotCount, ...)`, then `PopulateStartingInventory` (`:334`) adds the deck, the relics and `StartingPotions`.

| | Ironclad | Silent | Defect | Regent | Necrobinder |
|---|---|---|---|---|---|
| class (`Chars/`) | `Ironclad.cs` | `Silent.cs` | `Defect.cs` | `Regent.cs` | `Necrobinder.cs` |
| `StartingHp` | 80 (`:33`) | 70 (`:34`) | 75 (`:27`) | 75 (`:32`) | 66 (`:32`) |
| `StartingGold` | 99 | 99 | 99 | 99 | 99 |
| `MaxEnergy` | 3 | 3 | 3 | 3 | 3 |
| `BaseOrbSlotCount` | 0 | 0 | **3** (`:63`) | 0 | 0 |
| `ShouldAlwaysShowStarCounter` | no | no | no | **yes** (`:36`) | no |
| `StartingPotions` | none | none | none | none | none |
| `UnlocksAfterRunAs` | null | null | Necrobinder | Silent | Regent |
| starter relic | `BurningBlood` (`:57`) | `RingOfTheSnake` (`:60`) | `CrackedCore` (`:53`) | `DivineRight` (`:58`) | `BoundPhylactery` (`:62`) |

No character overrides `MaxEnergy`, `StartingPotions` or the draw. Only Defect overrides `BaseOrbSlotCount`, and only Regent overrides `ShouldAlwaysShowStarCounter` (a UI flag; stars exist for every player). The other overrides, such as Defect's `EyelineOffset` (`Defect.cs:37`) and Necrobinder's `ExtraAssetPaths` (`Necrobinder.cs:42`), are art.

## 2. Starter decks (in order) and starter relics
| Character | Deck | Count |
|---|---|---|
| Ironclad (`Ironclad.cs:43-54`) | 5× `StrikeIronclad`, 4× `DefendIronclad`, `Bash` | 10 |
| Silent (`Silent.cs:44-57`) | 5× `StrikeSilent`, 5× `DefendSilent`, `Neutralize`, `Survivor` | 12 |
| Defect (`Defect.cs:39-50`) | 4× `StrikeDefect`, 4× `DefendDefect`, `Zap`, `Dualcast` | 10 |
| Regent (`Regent.cs:44-55`) | 4× `StrikeRegent`, 4× `DefendRegent`, `FallingStar`, `Venerate` | 10 |
| Necrobinder (`Necrobinder.cs:48-59`) | 4× `StrikeNecrobinder`, 4× `DefendNecrobinder`, `Bodyguard`, `Unleash` | 10 |

Starter relic effects (`Models.Relics/`):
- `BurningBlood:16`: `AfterCombatVictory` heals 6.
- `RingOfTheSnake:14`: `ModifyHandDraw` adds 2 cards on turn 1 (`Owner.PlayerCombatState.TurnNumber > 1` returns early).
- `CrackedCore:29`: `BeforeSideTurnStart` channels one `LightningOrb` (`:35`) on the owner's first turn only (`TurnNumber <= 1`, `:31`).
- `DivineRight:16`: `AfterRoomEntered` gains 3 stars (`StarsVar(3)`).
- `BoundPhylactery:16-42`: `SpawnsPets`; `SummonPet` calls `OstyCmd.Summon(Owner, Summon.BaseValue)` at combat start and at each player turn start.

Ascender's Bane is appended at A≥`AscendersBane` for every character (engine: `run/mod.rs:1703`).

## 3. Card pools
`Pools/<C>CardPool.cs` has `GenerateAllCards()` and `FilterThroughEpochs`. Every character's pool has exactly 9 epoch-gated cards: Epoch 2, 5 and 7 each gate 3. They are listed in `Epochs/<C>{2,5,7}Epoch.cs`. The engine assumes every epoch is revealed (`run/cards.rs:159`, `run/mod.rs:395`).

"Multiplayer-only" means `MultiplayerConstraint => MultiplayerOnly` (`inventory.md`).

| Rarity (A/S/P) | Ironclad | Silent | Defect | Regent | Necrobinder |
|---|---|---|---|---|---|
| Basic | 3 (2/1/0) | 4 (2/2/0) | 4 (1/3/0) | 4 (2/2/0) | 4 (2/2/0) |
| Common | 20 (13/7/0) | 20 (9/11/0) | 20 (12/8/0) | 20 (9/11/0) | 20 (12/8/0) |
| Uncommon | 38 (15/14/9) | 38 (10/21/7) | 38 (8/20/10) | 38 (10/20/8) | 38 (12/16/10) |
| Rare | 27 (8/9/10) | 27 (4/13/10) | 27 (8/9/10) | 27 (9/8/10) | 27 (8/10/9) |
| Ancient | 2 (1/0/1) | 2 (1/0/1) | 2 (0/1/1) | 2 (1/0/1) | 2 (1/0/1) |
| **Total** (`GenerateAllCards` length) | 90 | 91 | 91 | 91 | 91 |
| Multiplayer-only | 5 | 5 | 5 | 5 | 5 |
| Single-player pool | 85 | 86 | 86 | 86 | 86 |

Notes:
- The engine's Necrobinder pool matches: 91 with 5 multiplayer-only gives 86 (`content/cards/pools.rs:261`, `run/cards.rs:429`).
- Ancient cards: Ironclad `Break`, `Corruption`; Silent `Suppress`, `WraithForm`; Defect `BiasedCognition`, `Quadcast`; Regent `MeteorShower`, `TheSealedThrone`; Necrobinder `ForbiddenGrimoire`, `Protector`. They are in the pool list but the engine excludes Ancient from transforms (`run/cards.rs:362`).
- Colorless is shared: 65 cards by script (25 Rare, 40 Uncommon, 12 of them multiplayer-only), pool `ColorlessCardPool.cs`.
- Character-conditional shared content (decomp, not in the pools):
  - `Relics/LastingCandy.cs:86` checks `Character is Ironclad`.
  - `Relics/ScrollBoxes.cs:70,110` checks `Character is Defect` and has an Ironclad pool fallback.
  - `Cards/Shiv.cs:71` checks `Character is Silent`, which is VFX only.
  - `Relics/SeaGlass.cs:79` and `Events/Orobas.cs:50` choose a card pool by `CharacterId`.
  - The Kaleidoscope Neow option needs every character pool (`run/neow.rs:99,167`).

## 4. Relic pools
`Pools/<C>RelicPool.cs`: 8 relics each, with the same shape for all five: 1 Starter, 1 Common, 2 Uncommon, 3 Rare and 1 Shop. `GetUnlockedRelics` removes the Epoch 3 and Epoch 6 relics (3 each) when unrevealed. The player's grab bag keeps Common, Uncommon, Rare and Shop only, so the starter is excluded (engine `run/relic_bag.rs:197`).

| Character | Starter | Common | Uncommon | Rare | Shop | Epoch 3 | Epoch 6 |
|---|---|---|---|---|---|---|---|
| Ironclad | BurningBlood | RedSkull | PaperPhrog, SelfFormingClay | CharonsAshes, DemonTongue, RuinedHelmet | Brimstone | RedSkull, PaperPhrog, RuinedHelmet | SelfFormingClay, CharonsAshes, DemonTongue |
| Silent | RingOfTheSnake | SneckoSkull | Tingsha, TwistedFunnel | HelicalDart, PaperKrane, ToughBandages | NinjaScroll | ToughBandages, PaperKrane, Tingsha | TwistedFunnel, SneckoSkull, HelicalDart |
| Defect | CrackedCore | DataDisk | GoldPlatedCables, SymbioticVirus | EmotionChip, PowerCell, Metronome | RunicCapacitor | DataDisk, SymbioticVirus, Metronome | GoldPlatedCables, EmotionChip, PowerCell |
| Regent | DivineRight | FencingManual | GalacticDust, Regalite | LunarPastry, MiniRegent, OrangeDough | VitruvianMinion | FencingManual, GalacticDust, LunarPastry | Regalite, MiniRegent, OrangeDough |
| Necrobinder | BoundPhylactery | BoneFlute | BookRepairKnife, FuneraryMask | BigHat, Bookmark, IvoryTile | UndyingSigil | BoneFlute, FuneraryMask, BookRepairKnife | IvoryTile, BigHat, Bookmark |

## 5. Potion pools (the Epochs gap)
`Pools/<C>PotionPool.cs:16-18` returns `<C>4Epoch.Potions`, so the potions are listed in `Epochs/<C>4Epoch.cs:24`, not in the pool class. `GetUnlockedPotions` returns nothing until `<C>4Epoch` is revealed (`:25-31`). Each character has exactly 3 potions, one per rarity, in this declaration order (rarity in brackets):

| Character | Potions |
|---|---|
| Ironclad (Ironclad4Epoch "The Pact") | `BloodPotion` [Common], `SoldiersStew` [Rare], `Ashwater` [Uncommon] |
| Silent (Silent4Epoch "Trophies") | `PoisonPotion` [Common], `GhostInAJar` [Rare], `CunningPotion` [Uncommon] |
| Defect (Defect4Epoch "Resolving Bugs") | `FocusPotion` [Common], `EssenceOfDarkness` [Rare], `PotionOfCapacity` [Uncommon] |
| Regent (Regent4Epoch "Grand Strategy") | `StarPotion` [Common], `CosmicConcoction` [Rare], `KingsCourage` [Uncommon] |
| Necrobinder (Necrobinder4Epoch "Spireborn") | `PotionOfDoom` [Common], `PotOfGhouls` [Rare], `BoneBrew` [Uncommon] |

The full reward pool is the character's 3 followed by the 45 `SharedPotionPool` entries. The engine's `NECROBINDER_POTION_OPTIONS` has 48 entries (`run/potions.rs:13`): the Necrobinder 3, then shared. `SharedPotionPool.GetUnlockedPotions` also prunes Potion1/Potion2 epoch potions (`:68-85`).

## 6. Per-character `Player` resources (decomp)
- **Orbs (Defect).** `PlayerCombatState.OrbQueue` (`Players/PlayerCombatState.cs:121`) is created in the constructor (`:137-139`): `Clear()`, then `AddCapacity(player.BaseOrbSlotCount)`.
  - `OrbQueue` (`Entities.Orbs/OrbQueue.cs`): `Capacity` (`:23`), `maxCapacity = 10` (`:15`), `Orbs` list, `TryEnqueue` (`:50`), `Insert`, `Remove`, `BeforeTurnEnd` (`:80`) and `AfterTurnStart` (`:92`) triggers.
  - Orb kinds are `Models.Orbs/{Dark,Frost,Glass,Lightning,Plasma}Orb.cs`. Commands are in `Commands/OrbCmd.cs`.
  - `Player.BaseOrbSlotCount` (`Players/Player.cs:216`) is saved, so Capacity changes persist across combats.
- **Stars (Regent).** `PlayerCombatState.Stars` (`:103`, set logs `History.StarsModified`), `GainStars`/`LoseStars` (`:211-226`), and `Hook.ShouldPayExcessEnergyCostWithStars` (`:194`). Cards can have a star cost (`:204`). `PlayerCmd.GainStars` is in `Commands/PlayerCmd.cs`.
- **Sovereign Blade/Forge (Regent).** `Cards/SovereignBlade.cs` has `CreatedThroughForge` (`:81`); `Commands/ForgeCmd.cs`. The blade is in `TokenCardPool`. Details go to S-MECH (`regent.md`).
- **Osty (Necrobinder).** `Player.Osty` = `PlayerCombatState.GetPet<Osty>()` (`Players/Player.cs:132`), plus `IsOstyAlive` and `IsOstyMissing` (`:137-142`). Pets are `PlayerCombatState._pets` (`Pets`, `:27`). A dead Osty stays in the list. `Models.Monsters/Osty.cs`; `Commands/OstyCmd.cs`.
- **`ExtraFields`** (`Players/ExtraPlayerFields.cs`): `CardShopRemovalsUsed`, `WongoPoints`, `CccomboBadgeUnlocked`, `DamageDealt`, `DebuffsApplied`. These are shared by every character.
- Ironclad and Silent have no extra resource, only starter-relic hooks and character-gated shared content (§3).

## 7. Character-specific `Player` fields and how the engine models them today
Engine files: `combat/state.rs` (`Player`, `CombatState`), `run/state.rs` (`RunState`), `combat/mod.rs` (`CombatConfig`).

| Game field | Varies by | Engine today | Gap for a superset `Player` |
|---|---|---|---|
| `Character` (`Player.cs:44`) | the 5 | none; `Necrobinder` is implied | add `Character` to `RunState` and `CombatConfig` |
| `StartingHp`/`MaxHp` | 80/70/75/75/66 | `RunState.hp/max_hp` (`run/state.rs:174-175`), `CombatConfig.player_hp/player_max_hp` (`combat/mod.rs:118-119`) | the consts are hardwired (§8) |
| `StartingGold` | 99 for all | `RunState.gold`, `CombatConfig.gold`, `Player.gold` (`state.rs:306`) | consts only |
| `MaxEnergy` | 3 for all | `RunState.max_energy` (`:177`), `CombatConfig.max_energy`, `Player.max_energy` (`state.rs:304`) | consts only |
| `BaseOrbSlotCount` | Defect 3 | none | add `orb_slots` (a persistent run field) and `orbs: Vec<Orb>` with `capacity` |
| `OrbQueue.Orbs`/`Capacity` | Defect | none | a new `Orb` type (5 kinds, passive and evoke state) |
| `Stars` | Regent | none (`state.rs:274`: "No in-scope card costs stars"); `EnergyCost` has no star part | add `stars: i32`, a star cost on cards, a history event |
| `Osty`/`_pets` | Necrobinder | `Player.pets: Vec<CreatureId>` (`state.rs:328`), `CombatState::osty()`/`is_osty_alive()` (`:554-566`), `Creature.pet_owner` (`:110`) | already generic; keep it |
| Sovereign Blade/Forge | Regent | none | the S-MECH spec |
| `PotionSlots` (`MaxPotionCount`) | none (3, `TightBelt` −1) | `RunState.potions: Vec<Option<&str>>`, `Player.potions` (`:317`) plus `unported_potions` (`:320`) | none |
| hand cap 10, draw 5 | none | `MAX_HAND_SIZE` (`combat/commands/piles.rs:12`), `HAND_DRAW` (`combat/turn.rs:12`) | none |
| `StartingPotions` | none (empty) | none | none |
| `RelicGrabBag` (player) | by relic pool | `RunState.player_relic_bag` (`:185`) | needs a character-pool constructor |
| `PlayerRng/Odds` | none | `RunState.player_rng` (`:152`) | none |
| `ExtraFields.CardShopRemovalsUsed` | none | `RunState.card_shop_removals_used` (`:187`) | none (the other 4 fields don't affect gameplay) |
| `IsActiveForHooks` | none | `Player.hooks_active` (`:333`) | none |

## 8. Where our code hardwires Necrobinder
(Tests omitted unless noted. Card, power and relic `content/` files named after Necrobinder-only cards are legitimately Necrobinder content and are not listed.)

**Stats and starting loadout**
- `combat/mod.rs:134-171` `CombatConfig::necrobinder` builds the deck (`:142-145`), the `BoundPhylactery` relic (`:156`), hp (`:163-164`), energy 3 (`:165`) and gold (`:166`). Consts `STARTING_MAX_HP = 66`, `STARTING_GOLD = 99` (`:201-204`). Also `:101` (doc), `examples/fight.rs:14`.
- `run/mod.rs:56-64` consts `STARTING_HP = 66`, `STARTING_GOLD`, `STARTING_ENERGY`. `:403` `RelicGrabBag::necrobinder`. `:405` and `:1703` `starting_deck` → `cards::starter_deck`. `:427-433` hp, gold, energy, `RunRelic::new(RelicKind::BoundPhylactery)`. `:382` is the doc.
- `run/cards.rs:402-408` `starter_deck()` (`STRIKE_NECROBINDER`, `DEFEND_NECROBINDER`, `BODYGUARD`, `UNLEASH`). `:103`/`:109` `is_strike`/`is_defend` compare these ids. `:56` `info()` looks up `NECROBINDER_CARD_POOL`. `:159-161` `NECROBINDER_CARD_POOL: [PoolCard; 91]`. `:398-400` `unlocked_cards()`. `:359-362` `transformation_options`.
- Callers of `unlocked_cards()`: `run/rewards.rs:5,195,198` and `run/shop.rs:8,170`.

**Pools**
- `run/relic_bag.rs:145` `NECROBINDER_RELIC_POOL` and `:197-205` `RelicGrabBag::necrobinder`.
- `run/potions.rs:13` `NECROBINDER_POTION_OPTIONS: [_; 48]`, used at `:76,85`. Other users: `run/shop.rs:14,302` and `content/events/{the_future_of_potions.rs:19, the_legends_were_true.rs:45, wellspring.rs:32}`.
- `content/cards/pools.rs:44-74` `NECROBINDER_ETHEREAL`, `NECROBINDER_NOT_GENERATED` and `necrobinder()`. `:133-145` picks the original card's pool for generated cards.
- Callers of `pools::necrobinder()`: potions `skill_potion.rs:41`, `power_potion.rs:41`, `attack_potion.rs:41`, `orobic_acid.rs:35`; cards `jackpot.rs:58`, `discovery.rs:58`, `abundance.rs:59`; powers `call_of_the_void.rs:36`, `calamity.rs:80`; relics `big_hat.rs:47`, `test_choose_a_card.rs:31`, `vexing_puzzlebox.rs:31`.
- `run/neow.rs:99,167` Kaleidoscope (needs every pool). `content/relics/scroll_boxes.rs:27` assumes "not Defect".

**Card and relic references by id**
- `content/relics/large_capsule.rs:26-27` adds `STRIKE_NECROBINDER`/`DEFEND_NECROBINDER`.
- `content/relics/strike_dummy.rs:25` and `ghost_seed.rs:24` match `CardKind::StrikeNecrobinder`/`DefendNecrobinder`.
- `content/cards/mod.rs:40,155,186-187,388-389,752-753` and `content/relics/mod.rs:20,182,349,518,777` register `BoundPhylactery`.
- `content/relics/bound_phylactery.rs`; `content/cards/{strike,defend}_necrobinder.rs`.

**Osty (pet) paths in core**
- `combat/state.rs:327-328,554-566`; `combat/commands/summon.rs:12-60` (`summon_osty`); `combat/commands/damage.rs:294-328` and `:512-513` (Osty absorbs and dies); `combat/turn.rs:339` (`pets.clear()`); `combat/hooks.rs:144-145` (`HpLossPhase::BeforeOsty/AfterOsty`, plus `:529-533,762-792,1403-1438,2216-2226`); `content/monsters/mod.rs:243` (`is_osty`); `content/monsters/osty.rs`.

**Tooling**
- `sts2-diff`: `session.rs:14` (`CHARACTER = "Necrobinder"`, used by `main.rs:104,192`, `session.rs:99`, `run_session.rs:131`, `run_session.rs:71` doc); `ids.rs:19-20`; `loadout.rs:3-7,87` (doc and ids); `greedy.rs:409` (the pets feature).
- `referee/referee.py:9,29,97,173,179,210,226` (`CHARACTER`; spec `"character"` already overrides it at `:97,173,210`); `tools/diff_map.sh:13`.
- `tools/inventory.py:23-25` pool names; `xtask/src/coverage.rs:1`; `docs/inventory/act1.{json,md}`.
- `sts2-tui`: `ui.rs:66`, `main.rs:1`, `README.md:3`.

**Not hardwired and already neutral**
- Potion belt size and `TightBelt` (`run/mod.rs:64-72`, `combat/mod.rs:157`), hand cap and draw count, and `RunState.max_energy`. The `RunState` and `CombatConfig` fields are plain values, so only the constructors need a `Character` argument.
