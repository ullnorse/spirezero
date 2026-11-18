//! The static action → entity table (`docs/plans/necro-e2e.md`, §1 "Policy head", WP1).
//!
//! For each index of the run space, the kind of thing it acts on and that thing's slot, so a
//! policy head can score an index from the token of its entity.
//!
//! Each row of [`ACTION_ENTITIES`] is [`ActionEntity`]: `kind` ([`EntityKind`]), `slot` (the hand
//! slot, belt slot, option, map column, reward, shop slot, bundle...), `sub` (a play's or potion's
//! target slot as in [`crate::action`]: 0 none or the player, 1-7 the enemies in order; a reward
//! card or alternative) and `phase` (the [`crate::run_obs::phase_code`] the index is legal in, or
//! [`ANY_PHASE`]). Which creature a target slot names depends on the fight; the table is fixed.

use crate::action::{
    DONE, END_TURN, HAND_SLOTS, OPTION_SLOTS as CHOICE_SLOTS, PICK_BASE, PLAY_BASE, POTION_BASE,
    POTION_SLOTS, TARGET_SLOTS,
};
use crate::run_action::{
    ALTERNATIVE_BASE, ALTERNATIVE_SLOTS, ANCIENT_BASE, BUNDLE_BASE, BUNDLE_SLOTS, BUY_CARD_BASE,
    BUY_POTION_BASE, BUY_RELIC_BASE, CANCEL_SELECT, CLAIM_BASE, COLUMN_SLOTS, CONFIRM_SELECT,
    DISCARD_POTION_BASE, EVENT_BASE, NODE_BASE, OPTION_SLOTS, PICK_CARD_BASE, PROCEED, REMOVE_CARD,
    REST_BASE, REWARD_CARD_SLOTS, REWARD_SLOTS, RUN_ACTIONS, SELECT_BASE, SELECT_SLOTS,
    SHOP_CARD_SLOTS, SHOP_POTION_SLOTS, SHOP_RELIC_SLOTS, SKIP_BASE, TAKE_RELIC, USE_POTION_BASE,
};

/// What an index acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EntityKind {
    /// Play the card in hand slot `slot` on target slot `sub`.
    HandCard = 0,
    /// Use the potion in belt slot `slot` on target slot `sub`, in a fight.
    CombatPotion = 1,
    EndTurn = 2,
    /// Option `slot` of a combat card choice.
    ChoiceOption = 3,
    /// Finish a combat card choice.
    ChoiceDone = 4,
    AncientOption = 5,
    EventOption = 6,
    /// The next row's point in column `slot`.
    MapColumn = 7,
    /// Claim reward `slot`.
    Reward = 8,
    /// Card `sub` of card reward `slot`.
    RewardCard = 9,
    /// Alternative `sub` (1 or 2) of card reward `slot`.
    RewardAlternative = 10,
    /// Skip reward `slot`.
    RewardSkip = 11,
    RestOption = 12,
    /// Use the potion in belt slot `slot` outside a fight.
    BeltPotion = 13,
    /// Option `slot` of a deck selection.
    SelectOption = 14,
    SelectConfirm = 15,
    SelectCancel = 16,
    TreasureRelic = 17,
    ShopCard = 18,
    ShopRelic = 19,
    ShopPotion = 20,
    ShopRemove = 21,
    Bundle = 22,
    Proceed = 23,
    /// Discard the potion in belt slot `slot` (RLN32).
    DiscardPotion = 24,
}

/// The phase of an index legal in several phases (a potion used outside a fight, proceed).
pub const ANY_PHASE: u8 = u8::MAX;

/// One row of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct ActionEntity {
    pub kind: EntityKind,
    pub slot: u8,
    pub sub: u8,
    pub phase: u8,
}

const fn row(kind: EntityKind, slot: usize, sub: usize, phase: u8) -> ActionEntity {
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "const fn; every slot is below 256"
    )]
    ActionEntity {
        kind,
        slot: slot as u8,
        sub: sub as u8,
        phase,
    }
}

/// Fills `n` rows from `base`: row `i` is `kind` at slot `i / per`, sub-slot `i % per`.
const fn block(
    mut table: [ActionEntity; RUN_ACTIONS],
    base: usize,
    n: usize,
    kind: EntityKind,
    per: usize,
    phase: u8,
) -> [ActionEntity; RUN_ACTIONS] {
    let mut i = 0;
    while i < n {
        table[base + i] = row(kind, i / per, i % per, phase);
        i += 1;
    }
    table
}

const fn build() -> [ActionEntity; RUN_ACTIONS] {
    use EntityKind as K;
    let combat = 3;
    let mut t = [row(K::Proceed, 0, 0, ANY_PHASE); RUN_ACTIONS];
    t = block(
        t,
        PLAY_BASE,
        HAND_SLOTS * TARGET_SLOTS,
        K::HandCard,
        TARGET_SLOTS,
        combat,
    );
    t = block(
        t,
        POTION_BASE,
        POTION_SLOTS * TARGET_SLOTS,
        K::CombatPotion,
        TARGET_SLOTS,
        combat,
    );
    t[END_TURN] = row(K::EndTurn, 0, 0, combat);
    t = block(t, PICK_BASE, CHOICE_SLOTS, K::ChoiceOption, 1, combat);
    t[DONE] = row(K::ChoiceDone, 0, 0, combat);
    t = block(t, ANCIENT_BASE, OPTION_SLOTS, K::AncientOption, 1, 0);
    t = block(t, EVENT_BASE, OPTION_SLOTS, K::EventOption, 1, 1);
    t = block(t, NODE_BASE, COLUMN_SLOTS, K::MapColumn, 1, 2);
    t = block(t, CLAIM_BASE, REWARD_SLOTS, K::Reward, 1, 4);
    t = block(
        t,
        PICK_CARD_BASE,
        REWARD_SLOTS * REWARD_CARD_SLOTS,
        K::RewardCard,
        REWARD_CARD_SLOTS,
        4,
    );
    t = block(
        t,
        ALTERNATIVE_BASE,
        REWARD_SLOTS * ALTERNATIVE_SLOTS,
        K::RewardAlternative,
        ALTERNATIVE_SLOTS,
        4,
    );
    // An alternative's `sub` is 1 or 2 (0 is skipping).
    let mut i = 0;
    while i < REWARD_SLOTS * ALTERNATIVE_SLOTS {
        t[ALTERNATIVE_BASE + i].sub += 1;
        i += 1;
    }
    t = block(t, SKIP_BASE, REWARD_SLOTS, K::RewardSkip, 1, 4);
    t = block(t, REST_BASE, OPTION_SLOTS, K::RestOption, 1, 7);
    t = block(
        t,
        USE_POTION_BASE,
        POTION_SLOTS,
        K::BeltPotion,
        1,
        ANY_PHASE,
    );
    t = block(t, SELECT_BASE, SELECT_SLOTS, K::SelectOption, 1, 5);
    t[CONFIRM_SELECT] = row(K::SelectConfirm, 0, 0, 5);
    t[CANCEL_SELECT] = row(K::SelectCancel, 0, 0, 5);
    t[TAKE_RELIC] = row(K::TreasureRelic, 0, 0, 8);
    t = block(t, BUY_CARD_BASE, SHOP_CARD_SLOTS, K::ShopCard, 1, 9);
    t = block(t, BUY_RELIC_BASE, SHOP_RELIC_SLOTS, K::ShopRelic, 1, 9);
    t = block(t, BUY_POTION_BASE, SHOP_POTION_SLOTS, K::ShopPotion, 1, 9);
    t[REMOVE_CARD] = row(K::ShopRemove, 0, 0, 9);
    t = block(t, BUNDLE_BASE, BUNDLE_SLOTS, K::Bundle, 1, 6);
    t[PROCEED] = row(K::Proceed, 0, 0, ANY_PHASE);
    t = block(
        t,
        DISCARD_POTION_BASE,
        POTION_SLOTS,
        K::DiscardPotion,
        1,
        ANY_PHASE,
    );
    t
}

/// The table, one row per index of the run space.
pub static ACTION_ENTITIES: [ActionEntity; RUN_ACTIONS] = build();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_index_has_one_entity_and_rows_are_distinct() {
        for (i, a) in ACTION_ENTITIES.iter().enumerate() {
            for b in &ACTION_ENTITIES[i + 1..] {
                assert_ne!(a, b, "index {i} shares its row");
            }
        }
        assert_eq!(
            ACTION_ENTITIES[PLAY_BASE + 3 * TARGET_SLOTS + 2],
            row(EntityKind::HandCard, 3, 2, 3)
        );
        assert_eq!(
            ACTION_ENTITIES[ALTERNATIVE_BASE + 2 * 2 + 1],
            row(EntityKind::RewardAlternative, 2, 2, 4)
        );
        assert_eq!(ACTION_ENTITIES[PROCEED].kind, EntityKind::Proceed);
    }
}
