//! sts2-core state to kernel state ([`Fight::from_core`]) and actions both ways.
//!
//! The projection keeps exactly what the supported content can read or change, and refuses any
//! state with anything else in it. Left out because nothing supported reads them: the combat
//! history (except whether a card play started this turn), events, gold and rewards, the other
//! RNG streams (the differential test checks they never move), each monster's own `Rng`,
//! `AmountOnTurnStart`, removed power instances, the turn phase and display-only fields.

use sts2_core::combat::state::{Card as CoreCard, CreatureId as CoreCreature, Side};
use sts2_core::combat::{Action as CoreAction, CardId, Decision, DecisionRequest, HistoryTag};
use sts2_core::content::afflictions::AfflictionKind;
use sts2_core::content::cards::CardData;
use sts2_core::content::monsters::MonsterKind;
use sts2_core::content::monsters::machine::ForcedMove;
use sts2_core::content::powers::PowerData;
use sts2_core::content::relics::RelicKind;
use sts2_core::rng::RunRngType;
use sts2_core::{Combat, Status as CoreStatus};

use crate::state::{
    Action, Card, Creature, Fight, Forced, MAX_CARDS, MAX_CREATURES, MAX_HAND, MAX_POWERS, Monster,
    NONE, Pile, Power, Status,
};
use crate::tables::{Affliction, Kernel, P, StateSpec, relic};

/// Why a state is outside the kernel's scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported(pub &'static str);

/// `Ok` if `ok`, else the reason.
const fn check(ok: bool, why: &'static str) -> Result<(), Unsupported> {
    if ok { Ok(()) } else { Err(Unsupported(why)) }
}

/// The sts2-core id of creature `i`.
fn c_id(i: usize) -> CoreCreature {
    CoreCreature(u32::try_from(i).expect("few creatures"))
}

fn index<T: TryInto<u8>>(value: T, why: &'static str) -> Result<u8, Unsupported> {
    value.try_into().map_err(|_| Unsupported(why))
}

impl Fight {
    /// The kernel state of an sts2-core combat, or why it is out of scope.
    ///
    /// # Errors
    /// The first unsupported thing in the state.
    #[allow(
        clippy::too_many_lines,
        reason = "one check per sts2-core state field, in declaration order"
    )]
    #[allow(
        clippy::many_single_char_names,
        reason = "`k`, `s`, `p`, `f` and `c` name the kernel, state, player, fight and creature \
                  throughout, as in sts2-core"
    )]
    pub fn from_core(k: &Kernel, combat: &Combat) -> Result<Self, Unsupported> {
        let s = combat.state();
        let status = match combat.status() {
            CoreStatus::PlayerTurn => Status::PlayerTurn,
            CoreStatus::Decision => Status::Decision,
            CoreStatus::Won => Status::Won,
            CoreStatus::Lost => Status::Lost,
            CoreStatus::Unsupported(_) => return Err(Unsupported("sts2-core stopped")),
        };
        let encounter = k
            .encounter_index(s.encounter)
            .ok_or(Unsupported("encounter"))?;
        let p = &s.player;
        let mut relics = 0;
        for r in &p.relics {
            check(!r.melted, "melted relic")?;
            relics |= match r.kind {
                RelicKind::BurningBlood => relic::BURNING_BLOOD,
                RelicKind::RingOfTheSnake => relic::RING_OF_THE_SNAKE,
                _ => return Err(Unsupported("relic")),
            };
        }
        check(p.potions.iter().all(Option::is_none), "potion")?;
        check(p.unported_potions.iter().all(Option::is_none), "potion")?;
        check(p.orbs.is_empty() && p.stars == 0, "orbs or stars")?;
        check(p.pets.is_empty() && !p.extra_turn, "pets or extra turn")?;
        check(!p.ready_to_end_turn, "ready to end the turn")?;
        check(p.creature == CoreCreature(0), "player creature id")?;
        check(s.allies == [CoreCreature(0)], "allies")?;
        check(s.escaped.is_empty(), "escaped creature")?;
        check(s.creatures.len() <= MAX_CREATURES, "too many creatures")?;
        check(s.cards.len() <= MAX_CARDS, "too many cards")?;

        let mut f = Self {
            rng_shuffle: s.rng[RunRngType::Shuffle].state(),
            rng_ai: s.rng[RunRngType::MonsterAi].state(),
            rng_niche: s.rng[RunRngType::Niche].state(),
            energy: p.energy,
            max_energy: p.max_energy,
            round: s.round,
            turn: p.turn_number,
            status,
            ascension: s.ascension,
            encounter,
            relics,
            enemy_side: s.side == Side::Enemy,
            in_progress: s.in_progress,
            pending_loss: s.pending_loss,
            hooks_active: s.creature(p.creature).is_alive(),
            played_this_turn: s
                .history_this_turn(HistoryTag::CardPlayStarted)
                .iter()
                .any(|e| e.actor == p.creature),
            pending: NONE,
            n_creatures: index(s.creatures.len(), "creatures")?,
            n_enemies: index(s.enemies.len(), "enemies")?,
            n_cards: index(s.cards.len(), "cards")?,
            next_uid: 0,
            enemies: [0; MAX_CREATURES - 1],
            hand: Pile::new(),
            play: Pile::new(),
            draw: Pile::new(),
            discard: Pile::new(),
            exhaust: Pile::new(),
            creatures: [Creature::default(); MAX_CREATURES],
            cards: [Card::default(); MAX_CARDS],
        };
        for (i, &e) in s.enemies.iter().enumerate() {
            f.enemies[i] = index(e.0, "creature id")?;
        }

        let slots = k.encounters[usize::from(encounter)].slots;
        for (i, c) in s.creatures.iter().enumerate() {
            check(c.id.0 as usize == i, "creature id")?;
            check(c.pet_owner.is_none(), "pet")?;
            check(c.powers.len() <= MAX_POWERS, "too many powers")?;
            let enemy = c.side == Side::Enemy;
            check(enemy == c.monster.is_some(), "creature side")?;
            check(enemy == (i != 0), "creature order")?;
            let mut creature = Creature {
                hp: c.hp,
                max_hp: c.max_hp,
                block: c.block,
                enemy,
                n_powers: index(c.powers.len(), "powers")?,
                slot: c
                    .slot
                    .and_then(|name| slots.iter().position(|&n| n == name))
                    .map_or(Ok(NONE), |i| index(i, "slot"))?,
                ..Creature::default()
            };
            for (j, power) in c.powers.iter().enumerate() {
                let kind = P::from_core(power.kind).ok_or(Unsupported("power"))?;
                let data = match (kind, *s.power_data(power.id)) {
                    (P::Slow, PowerData::Slow(d)) => d.slow_amount,
                    (P::Illusion, PowerData::Illusion(d)) => i32::from(d.is_reviving),
                    (P::Ritual, PowerData::Ritual(d)) => i32::from(d.was_just_applied_by_enemy),
                    (P::Skittish, PowerData::Skittish(d)) => {
                        i32::from(d.has_gained_block_this_turn)
                    }
                    (P::HardenedShell, PowerData::HardenedShell(d)) => d.damage_received_this_turn,
                    (P::Vigor, PowerData::Vigor(d)) => match d.command_to_modify {
                        None if d.amount_when_attack_started == 0 => 0,
                        // A monster's own attack (no card), remembered with its amount then.
                        Some(c)
                            if c.card.is_none()
                                && c.attacker == c_id(i)
                                && d.amount_when_attack_started > 0 =>
                        {
                            d.amount_when_attack_started
                        }
                        _ => return Err(Unsupported("Vigor command")),
                    },
                    (_, PowerData::None) => 0,
                    _ => return Err(Unsupported("power data")),
                };
                creature.powers[j] = Power {
                    kind,
                    skip_tick: power.skip_next_duration_tick,
                    applier: power.applier.map_or(Ok(NONE), |a| index(a.0, "applier"))?,
                    uid: f.next_uid,
                    amount: power.amount,
                    data,
                };
                f.next_uid += 1;
            }
            creature.remask();
            if let Some(m) = &c.monster {
                creature.monster = Self::project_monster(k, m)?;
            }
            f.creatures[i] = creature;
        }

        check(
            p.hand.len() <= MAX_HAND && p.play.len() <= MAX_HAND,
            "pile size",
        )?;
        let card_ids = |cards: &[CardId]| -> Result<Vec<u8>, Unsupported> {
            cards.iter().map(|c| index(c.0, "card id")).collect()
        };
        for card in card_ids(&p.hand)? {
            f.hand.push(card);
        }
        for card in card_ids(&p.draw)?.into_iter().rev() {
            f.draw.push(card);
        }
        for card in card_ids(&p.discard)? {
            f.discard.push(card);
        }
        for card in card_ids(&p.exhaust)? {
            f.exhaust.push(card);
        }
        for card in card_ids(&p.play)? {
            f.play.push(card);
        }
        for (i, c) in s.cards.iter().enumerate() {
            check(c.id.0 as usize == i, "card id")?;
            let spec = k
                .card_spec(c.kind, c.upgrade_level)
                .ok_or(Unsupported("card"))?;
            let fresh = CoreCard::new(c.id, c.kind, c.upgrade_level);
            check(c.keywords == fresh.keywords, "card keywords")?;
            check(c.energy_cost == fresh.energy_cost, "card energy cost")?;
            check(c.star_cost == fresh.star_cost, "card star cost")?;
            let lists = s.cost_lists();
            check(
                lists.energy(c.id).is_empty() && lists.star(c.id).is_empty(),
                "card cost lists",
            )?;
            check(
                !c.exhaust_on_next_play && !c.single_turn_retain && !c.single_turn_sly,
                "card flags",
            )?;
            check(
                c.enchantment.is_none() && c.base_replay_count == 0,
                "enchantment",
            )?;
            check(!c.is_dupe && c.clone_of.is_none(), "clone")?;
            check(c.data == CardData::None, "card data")?;
            let affliction = match c.affliction {
                None => Affliction::None,
                Some(a) if a.amount == 1 && a.kind == AfflictionKind::Ringing => {
                    Affliction::Ringing
                }
                Some(a) if a.amount == 1 && a.kind == AfflictionKind::Entangled => {
                    Affliction::Entangled
                }
                Some(_) => return Err(Unsupported("affliction")),
            };
            f.cards[i] = Card {
                spec,
                pile: NONE,
                affliction,
            };
        }
        for n in 0..5 {
            let cards: Vec<u8> = f.pile_cards(n).collect();
            for card in cards {
                f.cards[usize::from(card)].pile = n;
            }
        }

        if let Some(request) = combat.pending_decision() {
            let DecisionRequest::ChooseCards {
                options,
                min,
                max,
                picked,
                ..
            } = request;
            let play = f.play.as_slice();
            let choosing = play.len() == 1
                && k.cards[usize::from(f.card(play[0]).spec)].kind
                    == sts2_core::content::cards::CardKind::Survivor;
            check(
                *min == 1 && *max == 1 && picked.is_empty() && choosing,
                "decision",
            )?;
            check(*options == **p.hand, "decision options")?;
            f.pending = play[0];
        }
        Ok(f)
    }

    /// A monster's state.
    fn project_monster(
        k: &Kernel,
        m: &sts2_core::content::monsters::Monster,
    ) -> Result<Monster, Unsupported> {
        let kind = k.monster_index(m.kind.id()).ok_or(Unsupported("monster"))?;
        let spec = &k.monsters[usize::from(kind)];
        let machine = &m.machine;
        check(!m.is_performing_move, "mid-move")?;
        let current = index(machine.current, "state")?;
        check(
            usize::from(current) < spec.states.len()
                && spec.states.len() == m.kind.moves().states.len(),
            "move table",
        )?;
        if matches!(m.kind, MonsterKind::Nibbit(_)) {
            // The kernel never evaluates Nibbit's `INIT_MOVE`.
            check(
                !matches!(
                    spec.states[usize::from(current)],
                    StateSpec::Conditional { .. }
                ),
                "Nibbit before its first roll",
            )?;
        }
        let (forced, forced_follow_up, forced_performed) = match machine.forced {
            None => (Forced::None, 0, false),
            Some(f) => {
                check(f.must_perform_once, "forced move")?;
                let kind = match f.kind {
                    ForcedMove::Stunned => Forced::Stunned,
                    ForcedMove::IllusionRevive => Forced::Revive,
                    _ => return Err(Unsupported("forced move")),
                };
                (
                    kind,
                    index(f.follow_up, "state")?,
                    f.performed_at_least_once,
                )
            }
        };
        let log = &machine.log;
        let mut recent = [0; 3];
        for (slot, state) in recent.iter_mut().zip(log.recent(3)) {
            *slot = index(state, "state")?;
        }
        let mut used = 0_u16;
        for state in 0..128 {
            if log.contains(state) {
                check(state < 16, "state")?;
                used |= 1 << state;
            }
        }
        Ok(Monster {
            kind,
            current,
            forced,
            forced_follow_up,
            forced_performed,
            performed_first_move: machine.performed_first_move,
            spawned_this_turn: m.spawned_this_turn,
            performing: false,
            log_len: u8::try_from(log.len()).unwrap_or(u8::MAX),
            recent,
            used,
            aux: match m.kind {
                MonsterKind::Nibbit(n) => u8::from(n.is_front),
                MonsterKind::Toadpole(t) => u8::from(t.is_front),
                _ => 0,
            },
        })
    }
}

impl Action {
    /// The sts2-core action.
    #[must_use]
    pub fn to_core(self) -> CoreAction {
        match self {
            Self::Play { card, target } => CoreAction::PlayCard {
                card: CardId(u32::from(card)),
                target: (target != NONE).then(|| CoreCreature(u32::from(target))),
            },
            Self::EndTurn => CoreAction::EndTurn,
            Self::Pick(card) => CoreAction::Resolve(Decision::Pick(CardId(u32::from(card)))),
        }
    }

    /// The kernel action for an sts2-core one, if it has one.
    #[must_use]
    pub fn from_core(action: &CoreAction) -> Option<Self> {
        let id = |n: u32| u8::try_from(n).ok();
        match action {
            CoreAction::PlayCard { card, target } => Some(Self::Play {
                card: id(card.0)?,
                target: match target {
                    Some(t) => id(t.0)?,
                    None => NONE,
                },
            }),
            CoreAction::EndTurn => Some(Self::EndTurn),
            CoreAction::Resolve(Decision::Pick(card)) => Some(Self::Pick(id(card.0)?)),
            _ => None,
        }
    }
}
