//! `MadScience` (`Models.Cards/MadScience.cs`): the card `TinkerTime` builds. Event rarity, cost
//! 1, upgrade adds Innate. Its `Type` is its saved `TinkerTimeType` (target `AnyEnemy` for an
//! Attack, else `Self`; `GainsBlock` for a Skill): an Attack deals 12 (3 hits with the Violence
//! rider), a Skill gains 8 block, a Power applies its rider's power (Expertise: 2 Strength and 2
//! Dexterity; Curious: 1 `CuriousPower`; Improvement: 1 `ImprovementPower`). The Sapping (2 Weak,
//! 2 Vulnerable), Choking (6 `StranglePower`), Energized (2 energy), Wisdom (draw 3) and Chaos (a
//! random card of the owner's pool, free this turn) riders follow the base effect.
//!
//! The deck card keeps `TinkerTimeType` and `TinkerTimeRider` in its counter ([`counter`]).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType, ValueProps};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var, pools,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct MadScience;

/// `TinkerTime.RiderEffect`, in enum order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rider {
    #[default]
    None,
    Sapping,
    Violence,
    Choking,
    Energized,
    Wisdom,
    Chaos,
    Expertise,
    Curious,
    Improvement,
}

impl Rider {
    /// Every rider in enum order.
    pub const ALL: [Self; 10] = [
        Self::None,
        Self::Sapping,
        Self::Violence,
        Self::Choking,
        Self::Energized,
        Self::Wisdom,
        Self::Chaos,
        Self::Expertise,
        Self::Curious,
        Self::Improvement,
    ];
}

/// `MadScience`'s saved fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TinkerTimeType`; `None` is `CardType.None` (a card no event set up).
    pub tinker_time_type: Option<CardType>,
    /// `TinkerTimeRider`.
    pub tinker_time_rider: Rider,
}

/// The `CardType` values `TinkerTimeType` takes, by their C# enum value (`None` is 0).
const TYPES: [Option<CardType>; 7] = [
    None,
    Some(CardType::Attack),
    Some(CardType::Skill),
    Some(CardType::Power),
    Some(CardType::Status),
    Some(CardType::Curse),
    Some(CardType::Quest),
];

/// The deck card's counter for `data`: `TinkerTimeType`'s enum value plus 8 × the rider's.
#[must_use]
pub fn counter(data: Data) -> u32 {
    let ty = TYPES
        .iter()
        .position(|&t| t == data.tinker_time_type)
        .expect("every type is listed");
    let rider = Rider::ALL
        .iter()
        .position(|&r| r == data.tinker_time_rider)
        .expect("every rider is listed");
    u32::try_from(ty + 8 * rider).expect("small")
}

/// The fields a deck card's counter stands for ([`counter`]).
#[must_use]
pub fn data_from_deck(counter: u32) -> CardData {
    let counter = usize::try_from(counter).expect("u32 fits usize");
    CardData::MadScience(Data {
        tinker_time_type: TYPES.get(counter % 8).copied().flatten(),
        tinker_time_rider: Rider::ALL.get(counter / 8).copied().unwrap_or_default(),
    })
}

/// The card's fields (a fresh card's: `CardType.None`, no rider).
#[must_use]
pub fn data(card: &Card) -> Data {
    match card.data {
        CardData::MadScience(data) => data,
        CardData::None => Data {
            tinker_time_type: None,
            tinker_time_rider: Rider::None,
        },
        _ => unreachable!("a MadScience holds MadScience data"),
    }
}

/// `TargetType`: `AnyEnemy` for an Attack, else `Self`.
fn target_of(data: Data) -> TargetType {
    if data.tinker_time_type == Some(CardType::Attack) {
        TargetType::AnyEnemy
    } else {
        TargetType::Myself
    }
}

#[sts2_macros::hooks]
impl Hooks for MadScience {
    type Me = CardId;
}

impl CardModel for MadScience {
    fn name(&self) -> &'static str {
        "Mad Science"
    }

    fn cost(&self) -> i32 {
        1
    }

    /// The constructor's `CardType.Attack`. The canonical `Type` is `CardType.None`, which no
    /// [`CardType`] stands for; instances read [`CardModel::instance_card_type`].
    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    /// `TinkerTimeType` (`CardType.None`, which only a card no event set up has, reads as the
    /// constructor's Attack).
    fn instance_card_type(&self, card: &Card) -> CardType {
        data(card).tinker_time_type.unwrap_or(CardType::Attack)
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    /// The canonical card's `TinkerTimeType` is `CardType.None`, so `Self`.
    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn target_in(&self, s: &CombatState, me: CardId) -> TargetType {
        target_of(data(s.card(me)))
    }

    fn instance_gains_block(&self, card: &Card) -> bool {
        data(card).tinker_time_type == Some(CardType::Skill)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 12),
            CardVar::new(Var::Block, 8),
            CardVar::new(Var::Named("SappingWeak"), 2),
            CardVar::new(Var::Named("SappingVulnerable"), 2),
            CardVar::new(Var::Named("ViolenceHits"), 3),
            CardVar::new(Var::Named("ChokingDamage"), 6),
            CardVar::new(Var::Named("EnergizedEnergy"), 2),
            CardVar::new(Var::Named("WisdomCards"), 3),
            CardVar::new(Var::Named("ExpertiseStrength"), 2),
            CardVar::new(Var::Named("ExpertiseDexterity"), 2),
            CardVar::new(Var::Named("CuriousReduction"), 1),
        ];
        VARS
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let d = data(card);
        if target_of(d) == TargetType::AnyEnemy {
            assert!(play.target.is_some(), "an Attack MadScience needs a target");
        }
        let player = eng.s.player.creature;
        match d.tinker_time_type {
            Some(CardType::Attack) => {
                let target = play.target.expect("checked above");
                let hits = if d.tinker_time_rider == Rider::Violence {
                    card.count_var(Var::Named("ViolenceHits"))
                } else {
                    1
                };
                let damage = card.var(Var::Damage);
                eng.attack(
                    &Attack::new(
                        player,
                        AttackTargets::Single(target),
                        AttackDamage::Fixed(damage),
                        Some(play.card),
                    )
                    .with_hits(hits),
                )?;
            }
            Some(CardType::Skill) => {
                let block = card.var(Var::Block);
                eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
            }
            Some(CardType::Power) => execute_power(eng, play.card, d.tinker_time_rider)?,
            _ => panic!("MadScience is invalid type {:?}", d.tinker_time_type),
        }
        if matches!(
            d.tinker_time_rider,
            Rider::Sapping | Rider::Choking | Rider::Energized | Rider::Wisdom | Rider::Chaos
        ) {
            execute_rider(eng, play, d.tinker_time_rider)?;
        }
        Ok(())
    }
}

/// `ExecutePower`: the rider's power on the owner (`TriggerAnim` is visual).
fn execute_power(eng: &mut Engine<'_>, me: CardId, rider: Rider) -> Flow {
    let player = eng.s.player.creature;
    let var = |eng: &Engine<'_>, name| eng.s.card(me).var(Var::Named(name));
    match rider {
        Rider::Expertise => {
            let strength = var(eng, "ExpertiseStrength");
            eng.apply_power_from_card(
                Some(me),
                player,
                PowerKind::Strength,
                strength,
                Some(player),
            )?;
            let dexterity = var(eng, "ExpertiseDexterity");
            eng.apply_power_from_card(
                Some(me),
                player,
                PowerKind::Dexterity,
                dexterity,
                Some(player),
            )
        }
        Rider::Curious => {
            let amount = var(eng, "CuriousReduction");
            eng.apply_power_from_card(Some(me), player, PowerKind::Curious, amount, Some(player))
        }
        Rider::Improvement => eng.apply_power_from_card(
            Some(me),
            player,
            PowerKind::Improvement,
            Decimal::ONE,
            Some(player),
        ),
        _ => Ok(()),
    }
}

/// `ExecuteRider(rider, cardPlay.Target)`.
fn execute_rider(eng: &mut Engine<'_>, play: &CardPlay, rider: Rider) -> Flow {
    let player = eng.s.player.creature;
    let card = eng.s.card(play.card);
    match rider {
        Rider::Sapping => {
            let target = play.target.expect("a Sapping MadScience is an Attack");
            let weak = card.var(Var::Named("SappingWeak"));
            let vulnerable = card.var(Var::Named("SappingVulnerable"));
            eng.apply_power_from_card(
                Some(play.card),
                target,
                PowerKind::Weak,
                weak,
                Some(player),
            )?;
            eng.apply_power_from_card(
                Some(play.card),
                target,
                PowerKind::Vulnerable,
                vulnerable,
                Some(player),
            )
        }
        Rider::Choking => {
            let target = play.target.expect("a Choking MadScience is an Attack");
            let amount = card.var(Var::Named("ChokingDamage"));
            eng.apply_power_from_card(
                Some(play.card),
                target,
                PowerKind::Strangle,
                amount,
                Some(player),
            )
        }
        Rider::Energized => {
            let energy = card.int_var(Var::Named("EnergizedEnergy"));
            eng.gain_energy(Decimal::from(energy))
        }
        Rider::Wisdom => {
            let cards = card.int_var(Var::Named("WisdomCards"));
            eng.draw(Decimal::from(cards), false).map(drop)
        }
        Rider::Chaos => {
            let pool = pools::character(eng.s.player.character);
            let picked = eng.get_distinct_for_combat(&pool, 1)?;
            let card = *picked.first().expect("the character pool has a card");
            eng.s.set_to_free_this_turn(card);
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))
        }
        _ => unreachable!("{rider:?} is not an ExecuteRider rider"),
    }
}
