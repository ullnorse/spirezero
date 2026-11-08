//! `PaelsTooth` (`Models.Relics/PaelsTooth.cs`): Ancient (Pael); `AfterObtained` removes 5 chosen
//! upgradable deck cards (`FromDeckForRemoval` with `IsUpgradable`) and keeps them
//! (`SerializableCards`, in `Id.Entry` ordinal order); after every won combat one of them, drawn
//! on the player's `Rewards` stream, returns to the deck upgraded.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::rng::PlayerRngType;
use crate::run::{RoomType, RunCard, RunCtx, RunError, RunState, SelectPurpose};

/// `CardsVar(5)` (`cardsCount`).
const CARDS: usize = 5;

pub struct PaelsTooth;

/// `PaelsTooth`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `SerializableCards`, in list order: the first `len` slots.
    pub cards: [Option<RunCard>; CARDS],
}

impl Data {
    /// `SerializableCards.Count`.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cards.iter().filter(|c| c.is_some()).count()
    }

    /// Whether `SerializableCards` is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn list(&self) -> Vec<RunCard> {
        self.cards.iter().flatten().copied().collect()
    }

    fn set(&mut self, list: &[RunCard]) {
        self.cards = [None; CARDS];
        for (slot, &card) in self.cards.iter_mut().zip(list) {
            *slot = Some(card);
        }
    }
}

fn data_mut(s: &mut RunState, me: usize) -> &mut Data {
    let RelicData::PaelsTooth(data) = &mut s.relics[me].data else {
        unreachable!("a PaelsTooth has PaelsTooth data");
    };
    data
}

impl RelicModel for PaelsTooth {
    fn name(&self) -> &'static str {
        "Pael's Tooth"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PaelsTooth(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let options: Vec<usize> = s
            .removable_cards()
            .into_iter()
            .filter(|&i| s.deck[i].is_upgradable())
            .collect();
        ctx.select(SelectPurpose::Relic { relic: me }, options, CARDS)
    }

    /// The picks, ordered by `Id.Entry` (`OrderBy`, stable): each is kept (`MutableClone`), then
    /// removed from the deck.
    fn on_cards_selected(&self, s: &mut RunState, me: usize, cards: &[usize]) {
        let mut picks = cards.to_vec();
        picks.sort_by(|&a, &b| s.deck[a].id.cmp(s.deck[b].id));
        let mut kept = data_mut(s, me).list();
        for &i in &picks {
            kept.push(s.deck[i]);
        }
        data_mut(s, me).set(&kept);
        s.remove_cards(&picks);
    }

    fn after_combat_end_on_deck(&self, s: &mut RunState, me: usize, _room: RoomType) {
        let mut kept = data_mut(s, me).list();
        // `!Owner.Creature.IsDead`: the deck half only runs after a won fight.
        if kept.is_empty() {
            return;
        }
        let card = *s.player_rng[PlayerRngType::Rewards]
            .next_item(&kept)
            .expect("a kept card");
        let mut returned = card;
        returned.upgrade();
        s.add_card(returned);
        let i = kept
            .iter()
            .position(|&c| c == card)
            .expect("the drawn card");
        kept.remove(i);
        data_mut(s, me).set(&kept);
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsTooth {
    type Me = RelicId;
}
