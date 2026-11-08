//! Enchantments (`EnchantmentModel`): modifiers attached to a card. Each listens to hooks right
//! after its card's affliction (`CombatState.IterateHookListeners`); `Me` is the card's id.
//!
//! To add an enchantment: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates
//! the variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/enchantments.rs"));

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::CardType;
use crate::decimal::Decimal;

/// `EnchantmentStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EnchantmentStatus {
    /// `EnchantmentStatus.Normal`.
    #[default]
    Normal,
    /// `EnchantmentStatus.Disabled`.
    Disabled,
}

/// An enchantment on a card (`CardModel.Enchantment`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Enchantment {
    /// The enchantment.
    pub kind: EnchantmentKind,
    /// `Amount`.
    pub amount: i32,
    /// `Status`.
    pub status: EnchantmentStatus,
    /// `Momentum._extraDamage`: the damage its plays have added so far.
    pub extra_damage: i32,
}

impl Enchantment {
    /// A fresh enchantment (`EnchantInternal` with `Amount`).
    #[must_use]
    pub const fn new(kind: EnchantmentKind, amount: i32) -> Self {
        Self {
            kind,
            amount,
            status: EnchantmentStatus::Normal,
            extra_damage: 0,
        }
    }
}

/// An enchantment's static data and value modifiers (`EnchantmentModel`).
#[expect(
    unused_variables,
    reason = "default bodies ignore their arguments, like the C# virtual methods"
)]
pub(crate) trait EnchantmentModel: Hooks<Me = CardId> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English display name (`Title`).
    fn name(&self) -> &'static str;
    /// `CanEnchantCardType`.
    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        true
    }
    /// The extra checks of an override of `CanEnchant`, on top of the base ones.
    fn can_enchant_extra(&self, card: &Card) -> bool {
        true
    }
    /// `IsStackable`.
    fn is_stackable(&self) -> bool {
        false
    }
    /// `ShouldStartAtBottomOfDrawPile`: on turn 1 the card moves to the bottom of the draw pile
    /// before the Innate cards move to the top (`CombatManager.cs:905-919`).
    fn should_start_at_bottom_of_draw_pile(&self) -> bool {
        false
    }
    /// `OnEnchant`: changes the card the enchantment was applied to (`ModifyCard`).
    fn on_enchant(&self, card: &mut Card) {}
    /// `OnPlay(choiceContext, cardPlay)`: after the card's own `OnPlay`, once per play of `card`.
    fn on_play(&self, eng: &mut Engine<'_>, card: CardId, play: &CardPlay) -> Flow {
        Ok(())
    }
    /// `EnchantPlayCount(originalPlayCount)` of `enchantment`: the card's replay count.
    fn enchant_play_count(&self, enchantment: Enchantment, original: u32) -> u32 {
        original
    }
    /// `EnchantBlockAdditive(originalBlock)` of `enchantment`.
    fn enchant_block_additive(&self, enchantment: Enchantment, block: Decimal) -> Decimal {
        Decimal::ZERO
    }
    /// `EnchantBlockMultiplicative(originalBlock)` of `enchantment`.
    fn enchant_block_multiplicative(&self, enchantment: Enchantment, block: Decimal) -> Decimal {
        Decimal::ONE
    }
    /// `EnchantDamageAdditive(originalDamage, props)` of `enchantment`.
    fn enchant_damage_additive(
        &self,
        enchantment: Enchantment,
        damage: Decimal,
        props: ValueProps,
    ) -> Decimal {
        Decimal::ZERO
    }
    /// `EnchantDamageMultiplicative(originalDamage, props)` of `enchantment`.
    fn enchant_damage_multiplicative(
        &self,
        enchantment: Enchantment,
        damage: Decimal,
        props: ValueProps,
    ) -> Decimal {
        Decimal::ONE
    }
}

impl EnchantmentKind {
    /// [`EnchantmentModel::enchant_play_count`], statically dispatched.
    pub(crate) fn enchant_play_count(self, enchantment: Enchantment, original: u32) -> u32 {
        with_model!(self, m => m.enchant_play_count(enchantment, original))
    }

    /// The English display name.
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

    /// `ShouldStartAtBottomOfDrawPile`.
    #[must_use]
    pub fn should_start_at_bottom_of_draw_pile(self) -> bool {
        with_model!(self, m => m.should_start_at_bottom_of_draw_pile())
    }

    /// `CanEnchant(card)` in combat (`EnchantmentModel.cs:273-294`): not a Status, Curse or
    /// Quest card, the card type, the override's checks, and no other enchantment unless this
    /// one stacks onto the same kind. (The deck-only Unplayable check does not apply.)
    #[must_use]
    pub fn can_enchant(self, card: &Card) -> bool {
        let card_type = card.card_type();
        let m = self.model();
        !matches!(
            card_type,
            CardType::Status | CardType::Curse | CardType::Quest
        ) && m.can_enchant_card_type(card_type)
            && card
                .enchantment
                .is_none_or(|e| m.is_stackable() && e.kind == self)
            && m.can_enchant_extra(card)
    }
}

/// `EnchantmentModel.ModifyCard` on `card`'s enchantment: `OnEnchant` (then `RecalculateValues`,
/// which only refreshes display vars). Run when an enchantment is first applied, after a
/// downgrade, and for a deck card's combat clone (which carries the keywords it added).
pub(crate) fn modify_card(card: &mut Card) {
    if let Some(enchantment) = card.enchantment {
        enchantment.kind.model().on_enchant(card);
        // `RecalculateForUpgradeOrEnchant` (`EnchantmentModel.cs:363`).
        card.stale_calculated = None;
    }
}
