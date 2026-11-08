//! Relics: one file per relic, each a unit struct implementing [`RelicModel`] and
//! [`Hooks`](crate::combat::hooks::Hooks).
//!
//! To add a relic: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/relics.rs"));

mod display;
pub use display::RelicStatus;
pub(crate) mod egg_relic_helper;
mod text;

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::decimal::Decimal;
use crate::run::map::{ActMap, MapError, MapPointType};
use crate::run::{
    CardAlternatives, CardCreation, PendingReward, RestOption, Reward, RoomType, RunCard, RunCtx,
    RunError, RunState, Shop,
};

impl RelicKind {
    /// The English display name (`Title`).
    #[must_use]
    pub fn name(self) -> &'static str {
        with_model!(self, m => m.name())
    }

    /// Whether the model can raise a player choice (see `may_prompt` in the content API),
    /// statically dispatched: [`CombatState::may_prompt`](crate::combat::CombatState::may_prompt)
    /// asks every model on every step.
    #[must_use]
    pub fn may_prompt(self) -> bool {
        with_model!(self, m => m.may_prompt())
    }

    /// The fields of a freshly obtained instance.
    #[must_use]
    pub fn init_data(self) -> RelicData {
        with_model!(self, m => m.init_data())
    }

    /// `IsUsedUp` of an instance with `data`.
    #[must_use]
    pub fn is_used_up(self, data: &RelicData) -> bool {
        with_model!(self, m => m.is_used_up(data))
    }
}

/// The relic model's own instance fields (`[SavedProperty]` counters and private fields).
///
/// The game keeps one relic instance for the whole run, so every field outlives the combat
/// unless the relic resets it in a hook (`AfterCombatEnd`, `BeforeCombatStart`, ...). The run
/// stores the data ([`RunRelic::data`](crate::run::RunRelic)); in combat read it with
/// `s.relic(me).data`, write it with `s.relic_mut(me)`; in a run-level hook it is
/// `s.relics[me].data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RelicData {
    /// The relic has no fields.
    #[default]
    None,
    /// `Nunchaku`'s fields ([`nunchaku::Data`]).
    Nunchaku(nunchaku::Data),
    /// `FishingRod`'s fields ([`fishing_rod::Data`]).
    FishingRod(fishing_rod::Data),
    /// `LavaRock`'s fields ([`lava_rock::Data`]).
    LavaRock(lava_rock::Data),
    /// `SilverCrucible`'s fields ([`silver_crucible::Data`]).
    SilverCrucible(silver_crucible::Data),
    /// `WingedBoots`'s fields ([`winged_boots::Data`]).
    WingedBoots(winged_boots::Data),
    /// `BoneTea`'s fields ([`bone_tea::Data`]).
    BoneTea(bone_tea::Data),
    /// `EmberTea`'s fields ([`ember_tea::Data`]).
    EmberTea(ember_tea::Data),
    /// `TeaOfDiscourtesy`'s fields ([`tea_of_discourtesy::Data`]).
    TeaOfDiscourtesy(tea_of_discourtesy::Data),
    /// `SilkenTress`'s fields ([`silken_tress::Data`]).
    SilkenTress(silken_tress::Data),
    /// `Girya`'s fields ([`girya::Data`]).
    Girya(girya::Data),
    /// `LavaLamp`'s fields ([`lava_lamp::Data`]).
    LavaLamp(lava_lamp::Data),
    /// `LastingCandy`'s fields ([`lasting_candy::Data`]).
    LastingCandy(lasting_candy::Data),
    /// `SwordOfStone`'s fields ([`sword_of_stone::Data`]).
    SwordOfStone(sword_of_stone::Data),
    /// `VenerableTeaSet`'s fields ([`venerable_tea_set::Data`]).
    VenerableTeaSet(venerable_tea_set::Data),
    /// `ArtOfWar`'s fields ([`art_of_war::Data`]).
    ArtOfWar(art_of_war::Data),
    /// `BeatingRemnant`'s fields ([`beating_remnant::Data`]).
    BeatingRemnant(beating_remnant::Data),
    /// `BeltBuckle`'s fields ([`belt_buckle::Data`]).
    BeltBuckle(belt_buckle::Data),
    /// `BookOfFiveRings`'s fields ([`book_of_five_rings::Data`]).
    BookOfFiveRings(book_of_five_rings::Data),
    /// `BurningSticks`'s fields ([`burning_sticks::Data`]).
    BurningSticks(burning_sticks::Data),
    /// `CentennialPuzzle`'s fields ([`centennial_puzzle::Data`]).
    CentennialPuzzle(centennial_puzzle::Data),
    /// `HappyFlower`'s fields ([`happy_flower::Data`]).
    HappyFlower(happy_flower::Data),
    /// `JossPaper`'s fields ([`joss_paper::Data`]).
    JossPaper(joss_paper::Data),
    /// `Kunai`'s fields ([`kunai::Data`]).
    Kunai(kunai::Data),
    /// `Kusarigama`'s fields ([`kusarigama::Data`]).
    Kusarigama(kusarigama::Data),
    /// `LetterOpener`'s fields ([`letter_opener::Data`]).
    LetterOpener(letter_opener::Data),
    /// `LizardTail`'s fields ([`lizard_tail::Data`]).
    LizardTail(lizard_tail::Data),
    /// `Orichalcum`'s fields ([`orichalcum::Data`]).
    Orichalcum(orichalcum::Data),
    /// `OrnamentalFan`'s fields ([`ornamental_fan::Data`]).
    OrnamentalFan(ornamental_fan::Data),
    /// `PenNib`'s fields ([`pen_nib::Data`]).
    PenNib(pen_nib::Data),
    /// `Pendulum`'s fields ([`pendulum::Data`]).
    Pendulum(pendulum::Data),
    /// `Permafrost`'s fields ([`permafrost::Data`]).
    Permafrost(permafrost::Data),
    /// `Pocketwatch`'s fields ([`pocketwatch::Data`]).
    Pocketwatch(pocketwatch::Data),
    /// `RainbowRing`'s fields ([`rainbow_ring::Data`]).
    RainbowRing(rainbow_ring::Data),
    /// `Shuriken`'s fields ([`shuriken::Data`]).
    Shuriken(shuriken::Data),
    /// `TuningFork`'s fields ([`tuning_fork::Data`]).
    TuningFork(tuning_fork::Data),
    /// `UnsettlingLamp`'s fields ([`unsettling_lamp::Data`]).
    UnsettlingLamp(unsettling_lamp::Data),
    /// `Vambrace`'s fields ([`vambrace::Data`]).
    Vambrace(vambrace::Data),
    /// `DemonTongue`'s fields ([`demon_tongue::Data`]).
    DemonTongue(demon_tongue::Data),
    /// `RedSkull`'s fields ([`red_skull::Data`]).
    RedSkull(red_skull::Data),
    /// `RuinedHelmet`'s fields ([`ruined_helmet::Data`]).
    RuinedHelmet(ruined_helmet::Data),
    /// `Metronome`'s fields ([`metronome::Data`]).
    Metronome(metronome::Data),
    /// `EmotionChip`'s fields ([`emotion_chip::Data`]).
    EmotionChip(emotion_chip::Data),
    /// `GalacticDust`'s fields ([`galactic_dust::Data`]).
    GalacticDust(galactic_dust::Data),
    /// `MiniRegent`'s fields ([`mini_regent::Data`]).
    MiniRegent(mini_regent::Data),
    /// `Regalite`'s fields ([`regalite::Data`]).
    Regalite(regalite::Data),
    /// `MawBank`'s fields ([`maw_bank::Data`]).
    MawBank(maw_bank::Data),
    /// `PollinousCore`'s fields ([`pollinous_core::Data`]).
    PollinousCore(pollinous_core::Data),
    /// `PaelsTears`'s fields ([`paels_tears::Data`]).
    PaelsTears(paels_tears::Data),
    /// `PumpkinCandle`'s fields ([`pumpkin_candle::Data`]).
    PumpkinCandle(pumpkin_candle::Data),
    /// `GoldenCompass`'s fields ([`golden_compass::Data`]).
    GoldenCompass(golden_compass::Data),
    /// `PaelsEye`'s fields ([`paels_eye::Data`]).
    PaelsEye(paels_eye::Data),
    /// `PaelsLegion`'s fields ([`paels_legion::Data`]).
    PaelsLegion(paels_legion::Data),
    /// `PaelsTooth`'s fields ([`paels_tooth::Data`]).
    PaelsTooth(paels_tooth::Data),
    /// `PaelsWing`'s fields ([`paels_wing::Data`]).
    PaelsWing(paels_wing::Data),
    /// `ToyBox`'s fields ([`toy_box::Data`]).
    ToyBox(toy_box::Data),
    /// `BingBong`'s fields ([`bing_bong::Data`]).
    BingBong(bing_bong::Data),
    /// `WongosMysteryTicket`'s fields ([`wongos_mystery_ticket::Data`]).
    WongosMysteryTicket(wongos_mystery_ticket::Data),
    /// `VelvetChoker`'s fields ([`velvet_choker::Data`]).
    VelvetChoker(velvet_choker::Data),
    /// `DustyTome`'s fields ([`dusty_tome::Data`]).
    DustyTome(dusty_tome::Data),
    /// `SeaGlass`'s fields ([`sea_glass::Data`]).
    SeaGlass(sea_glass::Data),
    /// `TouchOfOrobas`'s fields ([`touch_of_orobas::Data`]).
    TouchOfOrobas(touch_of_orobas::Data),
    /// `ArchaicTooth`'s fields ([`archaic_tooth::Data`]).
    ArchaicTooth(archaic_tooth::Data),
    /// `BrilliantScarf`'s fields ([`brilliant_scarf::Data`]).
    BrilliantScarf(brilliant_scarf::Data),
    /// `FurCoat`'s fields ([`fur_coat::Data`]).
    FurCoat(fur_coat::Data),
    /// `IronClub`'s fields ([`iron_club::Data`]).
    IronClub(iron_club::Data),
    /// `ThrowingAxe`'s fields ([`throwing_axe::Data`]).
    ThrowingAxe(throwing_axe::Data),
    /// `MusicBox`'s fields ([`music_box::Data`]).
    MusicBox(music_box::Data),
    /// `TestMapMarker`'s fields ([`test_map_marker::Data`]).
    #[cfg(test)]
    TestMapMarker(test_map_marker::Data),
}

/// A relic as the combat receives it: its model and its fields as of combat start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OwnedRelic {
    /// The relic.
    pub kind: RelicKind,
    /// Its fields.
    pub data: RelicData,
    /// `IsMelted`: owned but no hook listener.
    pub melted: bool,
}

impl From<RelicKind> for OwnedRelic {
    /// A freshly obtained relic.
    fn from(kind: RelicKind) -> Self {
        Self {
            kind,
            data: kind.init_data(),
            melted: false,
        }
    }
}

/// A relic's lifecycle methods (`RelicModel`) and its run-level hooks.
///
/// The run-level hooks are dispatched by the run layer over the owned relics in order
/// (`run/hooks.rs`); `me` is the relic's index in [`RunState::relics`]. A hook the combat
/// engine also dispatches during a fight (`Hooks`) has an `_out_of_combat` twin here.
#[expect(
    unused_variables,
    reason = "default bodies ignore their arguments, like the C# virtual methods"
)]
pub(crate) trait RelicModel: Hooks<Me = RelicId> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English display name (`Title`).
    fn name(&self) -> &'static str;

    /// The instance fields of a freshly obtained relic (the C# field initializers).
    fn init_data(&self) -> RelicData {
        RelicData::None
    }

    /// `IsUsedUp` of an instance with `data`.
    fn is_used_up(&self, data: &RelicData) -> bool {
        false
    }

    /// Whether the run can own the relic: `false` while something it can do is not ported, so
    /// obtaining it is [`RunError::UnsupportedRelic`].
    fn obtainable(&self) -> bool {
        true
    }

    /// `IsAllowed(runState)`: whether the relic bags may still give it; a bag culls the relics
    /// that fail it before every pull (`RelicGrabBag.RemoveDisallowedRelicsFromDeques`). Most
    /// overrides are [`is_before_act3_treasure_chest`](crate::run::relic_bag::is_before_act3_treasure_chest).
    fn is_allowed(&self, s: &RunState) -> bool {
        true
    }

    /// `AfterObtained`: when the relic joins the run (`RelicCmd.Obtain`). A prompt it opens
    /// ([`RunCtx::select`], [`RunCtx::offer_rewards`]) runs once the hook returns.
    ///
    /// # Errors
    /// If it meets content that is not ported.
    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        Ok(())
    }

    /// `AfterRoomEntered(room)` for a room without a fight (the combat engine dispatches the
    /// fight's at combat start).
    fn after_room_entered_out_of_combat(&self, s: &mut RunState, me: usize, room: RoomType) {}

    /// `AfterRoomEntered(room)` of a merchant room for the relics that act on its inventory
    /// (`LordsParasol`), after every relic's [`RelicModel::after_room_entered_out_of_combat`]:
    /// queue entries in [`Shop::free_purchases`], which the run then buys in order.
    fn after_merchant_room_entered(&self, s: &RunState, me: usize, shop: &mut Shop) {}

    /// `AfterRoomEntered(room)` of a fight room entered from the map, for the relics whose effect
    /// is on the run's HP: the run calls it right before the fight starts (`AfterRoomEntered` is
    /// the first hook of a combat room, so nothing in the fight has happened yet).
    fn after_map_combat_room_entered(&self, s: &mut RunState, me: usize, point: MapPointType) {}

    /// `AfterCombatVictory(room)` for the relics that act on the run (`RelicCmd.Replace`), run
    /// once the fight has handed the streams back, before the rewards.
    ///
    /// # Errors
    /// If it meets content that is not ported.
    fn after_combat_victory_in_run(
        &self,
        ctx: &mut RunCtx<'_>,
        me: usize,
        room: RoomType,
    ) -> Result<(), RunError> {
        Ok(())
    }

    /// Whether [`RelicModel::after_combat_victory_in_run`] may fail after the next fight won,
    /// as far as the run state `s` tells. A relic whose hook can fail must say so here:
    /// [`Run::accepts`](crate::run::Run::accepts) tries a combat action on a copy only then.
    fn victory_in_run_may_fail(&self, s: &RunState, me: usize) -> bool {
        false
    }

    /// `AfterCombatEnd(room)` for the relics that act on the deck (which the combat does not
    /// see): run once the fight has handed the streams back, before the rewards.
    fn after_combat_end_on_deck(&self, s: &mut RunState, me: usize, room: RoomType) {}

    /// `ShouldDieLate(creature)` for the player outside combat.
    fn should_die_late_out_of_combat(&self, s: &RunState, me: usize) -> bool {
        true
    }

    /// `AfterPreventingDeath(creature)` for the player outside combat, when this relic prevented it.
    fn after_preventing_death_out_of_combat(&self, s: &mut RunState, me: usize) {}

    /// `ShouldGenerateTreasure(player)`.
    fn should_generate_treasure(&self, s: &RunState, me: usize) -> bool {
        true
    }

    /// `ShouldForcePotionReward(player, roomType)`.
    fn should_force_potion_reward(&self, s: &RunState, me: usize, room: RoomType) -> bool {
        false
    }

    /// `TryModifyRewards(player, rewards, room)`: `rewards` is the populated list; rewards the
    /// hook adds go in `added` and are populated after every listener ran.
    fn try_modify_rewards(
        &self,
        s: &mut RunState,
        me: usize,
        room: Option<RoomType>,
        rewards: &[Reward],
        added: &mut Vec<PendingReward>,
    ) -> bool {
        false
    }

    /// `TryModifyRewardsLate`: after every `TryModifyRewards`, on the same list (the populated
    /// rewards, then those added so far, unpopulated).
    fn try_modify_rewards_late(
        &self,
        s: &mut RunState,
        me: usize,
        room: Option<RoomType>,
        rewards: &mut [Reward],
        added: &mut [PendingReward],
    ) -> bool {
        false
    }

    /// `TryModifyCardRewardAlternatives(player, cardReward, alternatives)`: an added
    /// [`CardAlternative::Relic`](crate::run::CardAlternative::Relic) runs [`RelicModel::on_card_reward_alternative`] when chosen.
    fn try_modify_card_reward_alternatives(
        &self,
        s: &RunState,
        me: usize,
        alternatives: &mut CardAlternatives,
    ) -> bool {
        false
    }

    /// The `OnSelect` of the [`CardAlternative::Relic`](crate::run::CardAlternative::Relic) this relic added.
    ///
    /// # Errors
    /// If it meets content that is not ported.
    fn on_card_reward_alternative(
        &self,
        ctx: &mut RunCtx<'_>,
        me: usize,
        option: &'static str,
    ) -> Result<(), RunError> {
        unreachable!("{} adds no card reward alternative {option}", self.name())
    }

    /// `AfterModifyingRewards`: the listeners whose `TryModifyRewards` returned true.
    fn after_modifying_rewards(&self, s: &mut RunState, me: usize) {}

    /// `BeforeCombatRewardOffered(rewards, room)`.
    fn before_combat_reward_offered(
        &self,
        s: &mut RunState,
        me: usize,
        rewards: &mut Vec<Reward>,
        room: RoomType,
    ) {
    }

    /// `ModifyCardRewardCreationOptions(player, options)`.
    fn modify_card_reward_creation_options(
        &self,
        s: &RunState,
        me: usize,
        options: CardCreation,
    ) -> CardCreation {
        options
    }

    /// `ModifyCardRewardCreationOptionsLate`.
    fn modify_card_reward_creation_options_late(
        &self,
        s: &RunState,
        me: usize,
        options: CardCreation,
    ) -> CardCreation {
        options
    }

    /// `TryModifyCardRewardOptions(player, cards, options)`.
    fn try_modify_card_reward_options(
        &self,
        s: &mut RunState,
        me: usize,
        cards: &mut Vec<RunCard>,
        options: CardCreation,
    ) -> bool {
        false
    }

    /// `TryModifyCardRewardOptionsLate`.
    fn try_modify_card_reward_options_late(
        &self,
        s: &mut RunState,
        me: usize,
        cards: &mut Vec<RunCard>,
        options: CardCreation,
    ) -> bool {
        false
    }

    /// `AfterModifyingCardRewardOptions`: the listeners whose `TryModifyCardRewardOptions`
    /// returned true.
    fn after_modifying_card_reward_options(&self, s: &mut RunState, me: usize) {}

    /// `TryModifyCardBeingAddedToDeck(card, out newCard)`: the replacement, if any.
    fn try_modify_card_being_added_to_deck(
        &self,
        s: &RunState,
        me: usize,
        card: RunCard,
    ) -> Option<RunCard> {
        None
    }

    /// `TryModifyCardBeingAddedToDeckLate`.
    fn try_modify_card_being_added_to_deck_late(
        &self,
        s: &RunState,
        me: usize,
        card: RunCard,
    ) -> Option<RunCard> {
        None
    }

    /// `ModifyMerchantCardCreationResults(player, cards)`: the cards a shop's card slot has just
    /// stocked (and again after every purchase).
    fn modify_merchant_card_creation_results(
        &self,
        s: &RunState,
        me: usize,
        cards: &mut [RunCard],
    ) {
    }

    /// `ModifyMerchantPrice(player, entry, price)` (no in-scope listener reads the entry).
    fn modify_merchant_price(&self, s: &RunState, me: usize, price: Decimal) -> Decimal {
        price
    }

    /// `ShouldRefillMerchantEntry(entry, player)`.
    fn should_refill_merchant_entry(&self, s: &RunState, me: usize) -> bool {
        false
    }

    /// `BeforeCardRemoved(card)`: a card is about to leave the deck.
    fn before_card_removed(&self, s: &mut RunState, me: usize, card: RunCard) {}

    /// `AfterCardChangedPiles(card, ..)` where the card's pile is now the deck: it was added
    /// to the deck, or is a deck transform's replacement (`card` as it joined).
    fn after_card_added_to_deck(&self, s: &mut RunState, me: usize, card: RunCard) {}

    /// The picks of the relic's own deck selection ([`SelectPurpose::Relic`]), as deck indices
    /// before any change, in pick order (`PaelsTooth`).
    ///
    /// [`SelectPurpose::Relic`]: crate::run::SelectPurpose::Relic
    fn on_cards_selected(&self, s: &mut RunState, me: usize, cards: &[usize]) {}

    /// `TryModifyRestSiteOptions(player, options)`.
    fn try_modify_rest_site_options(
        &self,
        s: &RunState,
        me: usize,
        options: &mut Vec<RestOption>,
    ) -> bool {
        false
    }

    /// `ShouldDisableRemainingRestSiteOptions(player)`: every listener must agree.
    fn should_disable_remaining_rest_site_options(&self, s: &RunState, me: usize) -> bool {
        true
    }

    /// `ModifyRestSiteHealAmount(creature, amount)`.
    fn modify_rest_site_heal_amount(&self, s: &RunState, me: usize, amount: Decimal) -> Decimal {
        amount
    }

    /// `TryModifyRestSiteHealRewards(player, rewards, isMimicked)`: rewards a heal also offers.
    fn try_modify_rest_site_heal_rewards(
        &self,
        s: &RunState,
        me: usize,
        rewards: &mut Vec<PendingReward>,
        is_mimicked: bool,
    ) -> bool {
        false
    }

    /// `ModifyHpLostAfterOsty(target, amount, ...)` for damage to the player outside combat
    /// (no dealer, no card source).
    fn modify_hp_lost_out_of_combat(&self, s: &RunState, me: usize, amount: Decimal) -> Decimal {
        amount
    }

    /// `AfterRestSiteHeal(player, isMimicked)`.
    fn after_rest_site_heal(&self, s: &mut RunState, me: usize, is_mimicked: bool) {}

    /// `AfterRestSiteSmith(player)`.
    fn after_rest_site_smith(&self, s: &mut RunState, me: usize) {}

    /// `ModifyGoldGained(player, amount)` outside combat.
    fn modify_gold_gained_out_of_combat(
        &self,
        s: &RunState,
        me: usize,
        amount: Decimal,
    ) -> Decimal {
        amount
    }

    /// `AfterModifyingGoldGained(player, amount)` outside combat: the listeners that changed
    /// `(int)` gold.
    fn after_modifying_gold_gained_out_of_combat(
        &self,
        s: &mut RunState,
        me: usize,
        amount: Decimal,
    ) {
    }

    /// `AfterGoldGained(player)` outside combat.
    fn after_gold_gained_out_of_combat(&self, s: &mut RunState, me: usize) {}

    /// `AfterItemPurchased(player, itemPurchased, goldSpent)`: after a shop purchase (card,
    /// relic, potion or card removal) and its restock. No in-scope listener reads the item.
    /// `ShouldProcurePotion(potion, player)` outside combat (`PotionCmd.TryToProcure`: potion
    /// rewards, shop potions, events): `false` vetoes (`Sozu`). No listener reads the potion.
    fn should_procure_potion_out_of_combat(&self, s: &RunState, me: usize) -> bool {
        true
    }

    fn after_item_purchased(&self, s: &mut RunState, me: usize, gold_spent: i32) {}

    /// `ModifyGeneratedMap(runState, map, actIndex)`: the freshly generated map of act
    /// `act_index`; replace it with `*map = ..` (`GoldenCompass`: [`ActMap::golden_path`]).
    /// Runs whenever a map is generated: entering an act, and [`RunCtx::regenerate_map`].
    ///
    /// # Errors
    /// Where generating the replacement throws.
    fn modify_generated_map(
        &self,
        s: &mut RunState,
        me: usize,
        map: &mut ActMap,
        act_index: usize,
    ) -> Result<(), MapError> {
        Ok(())
    }

    /// `ModifyGeneratedMapLate(runState, map, actIndex)`: after every listener's
    /// `ModifyGeneratedMap`, for marks on the final map (`FurCoat`).
    ///
    /// # Errors
    /// As [`RelicModel::modify_generated_map`].
    fn modify_generated_map_late(
        &self,
        s: &mut RunState,
        me: usize,
        map: &mut ActMap,
        act_index: usize,
    ) -> Result<(), MapError> {
        Ok(())
    }

    /// `ModifyUnknownMapPointRoomTypes(roomTypes)`: the room types a `?` can roll.
    fn modify_unknown_map_point_room_types(
        &self,
        s: &RunState,
        me: usize,
        types: &mut Vec<RoomType>,
    ) {
    }

    /// `ModifyNextEvent(event)`: the event a `?` room is about to start.
    fn modify_next_event(&self, s: &RunState, me: usize, event: &'static str) -> &'static str {
        event
    }
}
