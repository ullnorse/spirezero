# Coverage exclusions

Entities that `cargo xtask coverage` does not count, one line per id: `- kind:Id reason`.
An optional `@scope` list right after the id limits a line to some characters or acts
(`@Ironclad,Silent`, `@overgrowth`, `@!Silent` for all but Silent; no list means every scope).
A reason starting with `check` is doubtful: the entity is excluded for now, but the report notes it
if it is unported. Resolve those by deleting the line (counts it) or rewording the reason (keeps it out).

Scope is a single-player run through Act 1 (Overgrowth or Underdocks, with Neow), Act 2 (Hive) or Act 3 (Glory), the latter two with their ancients and the shared ancient Darv, as one of the five characters; unscoped lines were confirmed for Necrobinder Overgrowth. Acts are `overgrowth`, `underdocks`, `hive`, `glory`; `@!hive,!glory` means "Act 1 only", `@!hive` "not in Hive". Every line below was
confirmed in the decomp (paths are under `ref/decomp/MegaCrit.Sts2.Core.`, `M.` = `Models.`): pool
membership, `IsAllowed`, act gates and the references of the entities that give it out.

Always excluded without a line here: inventory entities with `multiplayer_only: true`
(17 multiplayer-only cards, `MassiveScroll` whose `IsAllowed` needs `Players.Count > 1`, and the
powers `Covered`, `Intercept` and `Knockdown` that use `NetService`).


## Events that some acts never offer (`IsAllowed`)

- event:CrystalSphere @!hive,!glory act-gated, needs `CurrentActIndex > 0` (M.Events/CrystalSphere.cs)
- event:DollRoom @!hive act-gated, needs `CurrentActIndex == 1` (M.Events/DollRoom.cs)
- event:FakeMerchant @!hive,!glory act-gated, needs `CurrentActIndex >= 1` (M.Events/FakeMerchant.cs)
- event:PotionCourier @!hive,!glory act-gated, needs `CurrentActIndex > 0` (M.Events/PotionCourier.cs)
- event:RanwidTheElder @overgrowth,underdocks act-gated, `CurrentActIndex == 0` is refused (M.Events/RanwidTheElder.cs)
- event:RelicTrader @overgrowth,underdocks act-gated, `CurrentActIndex == 0` is refused (M.Events/RelicTrader.cs)
- event:StoneOfAllTime @!hive act-gated, needs `CurrentActIndex == 1` (M.Events/StoneOfAllTime.cs)
- event:Symbiote @!hive,!glory act-gated, needs `CurrentActIndex > 0` (M.Events/Symbiote.cs)
- event:WelcomeToWongos @!hive act-gated, needs `CurrentActIndex == 1` (M.Events/WelcomeToWongos.cs)
- event:WarHistorianRepy act-gated, `IsAllowed` always returns false (M.Events/WarHistorianRepy.cs)

## Given out only by an excluded event

- relic:FakeAnchor @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeBloodVial @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeHappyFlower @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeLeesWaffle @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeMango @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeMerchantsRug @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeOrichalcum @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeSneckoEye @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeStrikeDummy @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- relic:FakeVenerableTeaSet @!hive,!glory Event-rarity relic; only FakeMerchant gives it (M.Events/FakeMerchant.cs), act 2+
- encounter:FakeMerchantEventEncounter @!hive,!glory event encounter of FakeMerchant (M.Events/FakeMerchant.cs), act 2+
- monster:FakeMerchantMonster @!hive,!glory monster of FakeMerchantEventEncounter (M.Encounters/FakeMerchantEventEncounter.cs)
- power:ConfusedPower @!hive,!glory only FakeSneckoEye applies it (M.Relics/FakeSneckoEye.cs)
- potion:FoulPotion @!hive,!glory Event-pool potion; only FakeMerchant and PotionCourier give it (M.Events/PotionCourier.cs), both act 2+
- relic:BingBong @!hive Event-rarity relic; only DollRoom gives it (M.Events/DollRoom.cs), act 2 only
- relic:DaughterOfTheWind @!hive Event-rarity relic; only DollRoom gives it (M.Events/DollRoom.cs), act 2 only
- relic:MrStruggles @!hive Event-rarity relic; only DollRoom gives it (M.Events/DollRoom.cs), act 2 only
- relic:WongoCustomerAppreciationBadge @!hive Event-rarity relic; only WelcomeToWongos gives it (M.Events/WelcomeToWongos.cs), act 2 only
- relic:WongosMysteryTicket @!hive Event-rarity relic; only WelcomeToWongos gives it (M.Events/WelcomeToWongos.cs), act 2 only
- relic:HistoryCourse Event-rarity relic; only WarHistorianRepy gives it (M.Events/WarHistorianRepy.cs), never allowed
- card:LanternKey @!hive Quest card; WarHistorianRepy (never allowed, M.Events/WarHistorianRepy.cs) and the Hive event TheLanternKey (M.Events/TheLanternKey.cs) give it
- enchantment:Corrupted @!hive,!glory only Symbiote applies it (M.Events/Symbiote.cs), act 2+
- enchantment:Vigorous @!hive only StoneOfAllTime applies it (M.Events/StoneOfAllTime.cs), act 2 only

## Abstract bases without a port

- encounter:BattlewornDummyEventEncounter abstract base of BattlewornDummyEventV1/V2/V3Encounter, not instantiable (M.Encounters/BattlewornDummyEventEncounter.cs); the three variants are counted

## Other acts' relics

- relic:LoomingFruit @!glory Ancient rarity (never rolled, `RelicGrabBag` takes Common/Uncommon/Rare/Shop); only the Glory ancient Nonupeipe offers it (M.Events/Nonupeipe.cs, M.Acts/Glory.cs)
- relic:VeryHotCocoa @!hive Ancient rarity (never rolled); only the Hive ancient Tezcatara offers it (M.Events/Tezcatara.cs, M.Acts/Hive.cs)
- relic:FresnelLens @overgrowth Event rarity (never rolled); only DrowningBeacon gives it (M.Events/DrowningBeacon.cs), an Underdocks event, not in Overgrowth (M.Acts/Overgrowth.cs)
- relic:SwordOfJade @overgrowth,underdocks check: SwordOfStone (SunkenStatue, Act 1) turns into it after 5 elites (M.Relics/SwordOfStone.cs); Act 1 has 5 elite nodes (Map/MapPointTypeCounts.cs) but one path may not meet 5 after the event
- monster:Parafright @!hive summoned only by TheObscura of the Hive (M.Monsters/TheObscura.cs, M.Acts/Hive.cs); IllusionPower only type-checks it (M.Powers/IllusionPower.cs)

## Other characters' pools

- relic:PaperKrane @!Silent Silent relic (M.RelicPools/SilentRelicPool.cs); WeakPower only reads it when owned, nothing else gives it
- relic:PaperPhrog @!Ironclad Ironclad relic (M.RelicPools/IroncladRelicPool.cs); VulnerablePower only reads it when owned, nothing else gives it
- card:Claw @!Defect Defect card; ScrollBoxes uses it only `if (player.Character is Defect)` (M.Relics/ScrollBoxes.cs)
- card:StrikeIronclad @!Ironclad Ironclad card; LargeCapsule uses it only for `TestMode` Deprived (M.Relics/LargeCapsule.cs)
- card:DefendIronclad @!Ironclad Ironclad card; LargeCapsule (TestMode Deprived) and the Fasten tooltip fallback only (M.Relics/LargeCapsule.cs, M.Cards/Fasten.cs)

## Only reachable through multiplayer-only cards

Not reachable from any in-scope entity via `deps`.

- power:BeaconOfHopePower dependency of the multiplayer-only card BeaconOfHope (M.Cards/BeaconOfHope.cs)
- power:CacophonyPower dependency of the multiplayer-only card Cacophony (M.Cards/Cacophony.cs)
- power:CoordinatePower dependency of the multiplayer-only card Coordinate (M.Cards/Coordinate.cs)
- power:SoulboundPower dependency of the multiplayer-only card Soulbound (M.Cards/Soulbound.cs)
- power:TagTeamPower dependency of the multiplayer-only card TagTeam (M.Cards/TagTeam.cs)
- power:UnderworldPower dependency of the multiplayer-only card Underworld (M.Cards/Underworld.cs)
- relic:VakuuCardSelector not a relic: the `ICardSelector` helper `WhisperingEarring` pushes (`Relics/VakuuCardSelector.cs`); ported as `Engine::vakuu_selector` (`relics/whispering_earring.rs`)

## Referenced outside the models, reviewed and left out (AUD3)

Not inventory entities, so no lines above; found by sweeping every non-model decomp namespace
(commands, factories, rooms, rewards, runs, rest-site options, hooks) for ids the scan does not reach.
`tools/inventory.py` follows `SUPPORT_DIRS` (`Entities.RestSite`, `Rewards` without `RewardsSet`,
`Commands`, `Factories`) transitively; everything else it leaves out on purpose:

* `Rewards/RewardsSet.cs` names `Bludgeon`, `Vajra`, `BlockPotion` and the like only in `GetTutorialMonsterRewards`/the tutorial elite branch (tutorial run, not a normal one).
* `PaelsGrowth` (Clone rest option) and `PumpkinCandle` (Kindle rest option) are given only by the Hive ancients Pael and Tezcatara (`Models.Events/Pael.cs`, `Tezcatara.cs`, `Models.Acts/Hive.cs`), so `CloneRestSiteOption`/`KindleRestSiteOption` stay out of an Act 1 run.
* `TheArchitect` is started by `RunManager` after the last act; `Sovereign Blade` (`ForgeCmd`) and `HibernatePower` (`FrostOrb`) belong to Regent and Defect; `DeprecatedCard/Encounter/Event` and the `Mocks` are placeholders and test fixtures.
* `AscendersBane` and `Cruelty` are ported but not in the inventory (`AscensionManager` is an ascension-only reference).

- power:TankPower applied only by the multiplayer-only card Tank (Models.Cards/Tank.cs; PI5)
- power:ConcoctPower applied only by the multiplayer-only card Concoct (Models.Cards/Concoct.cs)
- power:FadePower applied only by the multiplayer-only card Fade (Models.Cards/Fade.cs)
- power:SneakyPower applied only by the multiplayer-only card Sneaky (Models.Cards/Sneaky.cs)
- power:HibernatePower applied only by the multiplayer-only card Hibernate (C/Hibernate.cs)
- power:OneForAllPower applied only by the multiplayer-only card OneForAll (C/OneForAll.cs)
- power:HammerTimePower applied only by the multiplayer-only card HammerTime (C/HammerTime.cs)
