//! The run's odds (`Odds/*.cs`): card-rarity pity, potion drops, the `?` room roll, and the
//! rarity rolls of the reward factories.
//!
//! The game computes all of these in C# `float`, so this is the run layer's isolated float
//! module (`docs/decisions.md`, "odds module"). Every value is an `f32` and every operation is
//! the single IEEE operation the C# performs, so results are bit-exact; nothing outside this
//! module sees a float except the traced odds values.
#![allow(
    clippy::float_arithmetic,
    clippy::float_cmp,
    clippy::suboptimal_flops,
    reason = "ports the game's f32 odds arithmetic and comparisons exactly; C# never fuses a \
              multiply and an add"
)]

use super::ascension::AscensionLevel;
use super::state::RoomType;
use crate::decimal::Decimal;
use crate::rng::Rng;

/// `10^n` as the exact `double` literals of .NET's `s_doublePowers10`, for `n <= 28`.
const DOUBLE_POWERS_10: [f64; 29] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22, 1e23, 1e24, 1e25, 1e26, 1e27, 1e28,
];

/// The `(decimal)float` cast (`DecCalc.VarDecFromR4`): the value rounded to 7 significant
/// digits, half to even, computed in `double` as .NET does.
///
/// # Panics
/// For a value of `1e28` or more (.NET throws `OverflowException`); game values are small.
#[must_use]
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the scaled value is below 1e7 and non-negative, as in .NET's `(uint)(int)` cast"
)]
pub fn decimal_from_f32(value: f32) -> Decimal {
    let biased = i32::try_from((value.to_bits() >> 23) & 0xFF).expect("8-bit exponent");
    let exp = biased - 126;
    if exp < -94 {
        return Decimal::ZERO;
    }
    assert!(exp <= 96, "(decimal)float: OverflowException");
    let mut dbl = f64::from(value.abs());
    // `log10(2) * 2^16 = 19728.3`.
    let mut power = 6 - ((exp * 19728) >> 16);
    let index = |p: i32| usize::try_from(p).expect("non-negative power");
    if power >= 0 {
        power = power.min(28);
        dbl *= DOUBLE_POWERS_10[index(power)];
    } else if power != -1 || dbl >= 1e7 {
        dbl /= DOUBLE_POWERS_10[index(-power)];
    } else {
        power = 0;
    }
    if dbl < 1e6 && power < 28 {
        dbl *= 10.0;
        power += 1;
    }
    let mantissa = i128::from(dbl.round_ties_even() as u32);
    let mantissa = if value < 0.0 { -mantissa } else { mantissa };
    if power < 0 {
        Decimal::new(mantissa * 10_i128.pow(power.unsigned_abs()), 0)
    } else {
        Decimal::new(mantissa, u8::try_from(power).expect("power <= 28"))
    }
}

/// `CardRarity`, as far as the reward pools use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardRarity {
    /// `CardRarity.Basic`.
    Basic,
    /// `CardRarity.Common`.
    Common,
    /// `CardRarity.Uncommon`.
    Uncommon,
    /// `CardRarity.Rare`.
    Rare,
    /// `CardRarity.Ancient`.
    Ancient,
}

impl CardRarity {
    /// `GetNextHighestRarityWithWrapping`: Basic → Common → Uncommon → Rare → Common; `None`
    /// for Ancient.
    #[must_use]
    pub const fn next_with_wrapping(self) -> Option<Self> {
        match self {
            Self::Basic | Self::Rare => Some(Self::Common),
            Self::Common => Some(Self::Uncommon),
            Self::Uncommon => Some(Self::Rare),
            Self::Ancient => None,
        }
    }
}

/// `CardRarityOddsType` without `Uniform` (no rarity roll is made for `Uniform`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardRarityOddsType {
    /// `CardRarityOddsType.RegularEncounter`.
    RegularEncounter,
    /// `CardRarityOddsType.EliteEncounter`.
    EliteEncounter,
    /// `CardRarityOddsType.BossEncounter`.
    BossEncounter,
    /// `CardRarityOddsType.Shop`.
    Shop,
}

impl CardRarityOddsType {
    /// `GetBaseOdds(type, Rare)` and `GetBaseOdds(type, Uncommon)`; `Scarcity` lowers the
    /// rare odds (`RegularRareOdds`, `EliteRareOdds`, `ShopRareOdds`).
    const fn base_odds(self, scarcity: bool) -> (f32, f32) {
        match (self, scarcity) {
            (Self::RegularEncounter, false) => (0.03, 0.37),
            (Self::RegularEncounter, true) => (0.0149, 0.37),
            (Self::EliteEncounter, false) => (0.1, 0.4),
            (Self::EliteEncounter, true) => (0.05, 0.4),
            (Self::BossEncounter, _) => (1.0, 0.0),
            (Self::Shop, false) => (0.09, 0.37),
            (Self::Shop, true) => (0.045, 0.37),
        }
    }

    /// `CardCreationOptions.ForRoom`'s odds type for a combat room.
    #[must_use]
    pub const fn for_room(room: RoomType) -> Self {
        match room {
            RoomType::Elite => Self::EliteEncounter,
            RoomType::Boss => Self::BossEncounter,
            _ => Self::RegularEncounter,
        }
    }
}

/// `CardRarityOdds._baseRarityOffset`.
const BASE_RARITY_OFFSET: f32 = -0.05;
/// `CardRarityOdds._maxRarityOffset`.
const MAX_RARITY_OFFSET: f32 = 0.4;
/// `CardRarityOdds.RarityGrowth` below and from `Scarcity`.
const RARITY_GROWTH: f32 = 0.01;
const SCARCITY_RARITY_GROWTH: f32 = 0.005;

/// `CardRarityOdds` (`Odds/CardRarityOdds.cs`): the card-reward pity offset. It draws on the
/// player's `rewards` stream.
#[derive(Clone, Copy, Debug)]
pub struct CardRarityOdds {
    /// `CurrentValue`.
    pub offset: f32,
    /// `AscensionLevel.Scarcity` is active.
    pub scarcity: bool,
}

impl CardRarityOdds {
    /// The odds at the start of a run at `ascension`.
    #[must_use]
    pub const fn new(ascension: u8) -> Self {
        Self {
            offset: BASE_RARITY_OFFSET,
            scarcity: AscensionLevel::Scarcity.is_active(ascension),
        }
    }

    /// `Roll(type)`: a rarity with the current offset (0 for a boss), then the pity update:
    /// a rare resets the offset, anything else grows it up to the cap.
    pub fn roll(&mut self, kind: CardRarityOddsType, rng: &mut Rng) -> CardRarity {
        let offset = if kind == CardRarityOddsType::BossEncounter {
            0.0
        } else {
            self.offset
        };
        let rarity = roll_rarity(kind, self.scarcity, offset, rng);
        let growth = if self.scarcity {
            SCARCITY_RARITY_GROWTH
        } else {
            RARITY_GROWTH
        };
        self.offset = if rarity == CardRarity::Rare {
            BASE_RARITY_OFFSET
        } else {
            (self.offset + growth).min(MAX_RARITY_OFFSET)
        };
        rarity
    }

    /// `RollWithoutChangingFutureOdds(type)`: a rarity with the current offset; the pity does
    /// not change (the shop's character cards).
    #[must_use]
    pub fn roll_without_changing_future_odds(
        &self,
        kind: CardRarityOddsType,
        rng: &mut Rng,
    ) -> CardRarity {
        roll_rarity(kind, self.scarcity, self.offset, rng)
    }

    /// `RollWithBaseOdds(type)`: the same thresholds with no offset; the pity does not change.
    pub fn roll_with_base_odds(&self, kind: CardRarityOddsType, rng: &mut Rng) -> CardRarity {
        roll_rarity(kind, self.scarcity, 0.0, rng)
    }
}

/// `RollWithoutChangingFutureOdds(type, offset)`: Rare below `rare + offset`, Uncommon below
/// `uncommon + (rare + offset)`, else Common. `RollWithBaseOdds` adds the two base odds without
/// an offset, which is the same `f32` sum.
fn roll_rarity(kind: CardRarityOddsType, scarcity: bool, offset: f32, rng: &mut Rng) -> CardRarity {
    let (rare, uncommon) = kind.base_odds(scarcity);
    let r = rng.next_float(1.0);
    let rare_threshold = rare + offset;
    if r < rare_threshold {
        CardRarity::Rare
    } else if r < uncommon + rare_threshold {
        CardRarity::Uncommon
    } else {
        CardRarity::Common
    }
}

/// `RollForUpgrade`'s base chance for a card reward.
pub const REWARD_UPGRADE_BASE: Decimal = Decimal::ZERO;
/// `RollForUpgrade`'s base chance at the merchant (`-999999999m`): it never upgrades.
pub const MERCHANT_UPGRADE_BASE: Decimal = Decimal::new(-999_999_999, 0);

/// What [`roll_for_upgrade`] needs to know about the card and the run.
#[derive(Clone, Copy, Debug)]
pub struct UpgradeRoll {
    /// `baseChance`: [`REWARD_UPGRADE_BASE`] or [`MERCHANT_UPGRADE_BASE`].
    pub base_chance: Decimal,
    /// `RunState.CurrentActIndex`.
    pub act_index: usize,
    /// The ascension level.
    pub ascension: u8,
    /// The card's rarity.
    pub rarity: CardRarity,
    /// `CardModel.IsUpgradable`.
    pub upgradable: bool,
}

/// `CardFactory.RollForUpgrade` (`CardFactory.cs:282-305`): whether the new card upgrades.
///
/// The `NextFloat()` is always drawn, before the upgradable check. A non-Rare card adds
/// `CurrentActIndex × UpgradedCardOddScaling` (`0.25m`, `0.125m` from `Scarcity` on) to the
/// base chance; then `Hook.ModifyCardRewardUpgradeOdds`, which no model overrides in this
/// game version, so it is the identity. The card upgrades if `(decimal)roll <= odds`.
///
/// # Panics
/// Never: the act index is at most 2.
pub fn roll_for_upgrade(roll: UpgradeRoll, rng: &mut Rng) -> bool {
    let r = decimal_from_f32(rng.next_float(1.0));
    if !roll.upgradable {
        return false;
    }
    let mut odds = roll.base_chance;
    if roll.rarity != CardRarity::Rare {
        let scaling = if AscensionLevel::Scarcity.is_active(roll.ascension) {
            Decimal::new(125, 3)
        } else {
            Decimal::new(25, 2)
        };
        let act = i32::try_from(roll.act_index).expect("act index is small");
        odds += Decimal::from(act) * scaling;
    }
    r <= odds
}

/// `EndlessConveyor.RollDish`'s weighted pick.
///
/// One `NextFloat()` scaled by the `float` sum of the weights, then the first entry whose running
/// `float` sum is above it. `None` if the roll lands on no entry (the event then keeps its
/// previous dish).
pub fn roll_weighted(weights: &[u8], rng: &mut Rng) -> Option<usize> {
    let total: f32 = weights.iter().map(|&w| f32::from(w)).sum();
    let roll = rng.next_float(1.0) * total;
    let mut sum: f32 = 0.0;
    for (i, &weight) in weights.iter().enumerate() {
        sum += f32::from(weight);
        if roll < sum {
            return Some(i);
        }
    }
    None
}

/// `PotionRewardOdds._basePotionRewardOdds`.
const BASE_POTION_ODDS: f32 = 0.4;
/// `PotionRewardOdds.eliteBonus`.
const ELITE_POTION_BONUS: f32 = 0.25;
/// The step the odds move by after each roll.
const POTION_ODDS_STEP: f32 = 0.1;

/// `PotionRewardOdds` (`Odds/PotionRewardOdds.cs`): the chance of a potion after a fight. It
/// draws on the player's `rewards` stream.
#[derive(Clone, Copy, Debug)]
pub struct PotionRewardOdds {
    /// `CurrentValue`.
    pub value: f32,
}

impl Default for PotionRewardOdds {
    fn default() -> Self {
        Self {
            value: BASE_POTION_ODDS,
        }
    }
}

impl PotionRewardOdds {
    /// `Roll(player, roomType)` with no `ShouldForcePotionReward` listener: a potion when
    /// `NextFloat() < odds + eliteBonus × 0.5`; the odds fall by 0.1 on a potion, else rise.
    pub fn roll(&mut self, room: RoomType, rng: &mut Rng) -> bool {
        let bonus = if room == RoomType::Elite {
            ELITE_POTION_BONUS
        } else {
            0.0
        };
        let threshold = self.value + bonus * 0.5;
        let dropped = rng.next_float(1.0) < threshold;
        if dropped {
            self.value -= POTION_ODDS_STEP;
        } else {
            self.value += POTION_ODDS_STEP;
        }
        dropped
    }
}

/// `Orobas._prismaticOdds` (`Models.Events/Orobas.cs:13`).
const OROBAS_PRISMATIC_ODDS: f32 = 0.333_333_3;

/// `Orobas.GenerateInitialOptions`' roll (`Models.Events/Orobas.cs:197`): `NextFloat() <
/// 0.3333333f` offers `PrismaticGem` instead of `SeaGlass`.
pub fn roll_orobas_prismatic(rng: &mut Rng) -> bool {
    rng.next_float(1.0) < OROBAS_PRISMATIC_ODDS
}

/// `PotionRarity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PotionRarity {
    /// `PotionRarity.Common`.
    Common,
    /// `PotionRarity.Uncommon`.
    Uncommon,
    /// `PotionRarity.Rare`.
    Rare,
}

/// `PotionFactory.CreateRandomPotions`' rarity roll: Rare at `<= 0.1`, Uncommon at `<= 0.35`.
pub fn roll_potion_rarity(rng: &mut Rng) -> PotionRarity {
    let r = rng.next_float(1.0);
    if r <= 0.1 {
        PotionRarity::Rare
    } else if r <= 0.35 {
        PotionRarity::Uncommon
    } else {
        PotionRarity::Common
    }
}

/// The rarities `RelicFactory.RollRarity` returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RolledRelicRarity {
    /// `RolledRelicRarity.Common`.
    Common,
    /// `RolledRelicRarity.Uncommon`.
    Uncommon,
    /// `RolledRelicRarity.Rare`.
    Rare,
}

/// `RelicFactory.RollRarity(rng)`: Common below 0.5, Uncommon below 0.83, else Rare.
pub fn roll_relic_rarity(rng: &mut Rng) -> RolledRelicRarity {
    let r = rng.next_float(1.0);
    if r < 0.5 {
        RolledRelicRarity::Common
    } else if r < 0.83 {
        RolledRelicRarity::Uncommon
    } else {
        RolledRelicRarity::Rare
    }
}

/// `Mathf.RoundToInt` / `(int)Mathf.Round` / `(int)Math.Round` of an `f32`: half to even.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "merchant prices are small positive values"
)]
const fn round_to_int(value: f32) -> i32 {
    value.round_ties_even() as i32
}

/// `MerchantCardEntry.CalcCost` before the sale.
///
/// `RoundToInt(GetCost(card) × NextFloat(0.95f, 1.05f))` on `shops`, where a colorless card's
/// base is first `RoundToInt(base × 1.15f)` (`Entities.Merchant/MerchantCardEntry.cs:38-51,118-129`).
pub fn merchant_card_cost(base: i32, colorless: bool, rng: &mut Rng) -> i32 {
    let base = if colorless {
        round_to_int(f32_of(base) * 1.15)
    } else {
        base
    };
    round_to_int(f32_of(base) * rng.next_float_range(0.95, 1.05))
}

/// `MerchantRelicEntry.CalcCost`: `(int)Math.Round((float)MerchantCost × NextFloat(0.85f,
/// 1.15f))` on `shops`; the `f32` product widens exactly to the `double` rounded.
pub fn merchant_relic_cost(merchant_cost: i32, rng: &mut Rng) -> i32 {
    round_to_int(f32_of(merchant_cost) * rng.next_float_range(0.85, 1.15))
}

/// `MerchantPotionEntry.CalcCost` (test mode off): `(int)Mathf.Round(base × NextFloat(0.95f,
/// 1.05f))` on `shops`.
pub fn merchant_potion_cost(base: i32, rng: &mut Rng) -> i32 {
    round_to_int(f32_of(base) * rng.next_float_range(0.95, 1.05))
}

/// `(float)int` for the small merchant bases.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "merchant bases are far below 2^24, so the cast is exact"
)]
const fn f32_of(value: i32) -> f32 {
    value as f32
}

/// The room types a `?` can become (`UnknownMapPointOdds._nonEventOdds` keys, then Event).
const UNKNOWN_BASE_ODDS: [(RoomType, f32); 4] = [
    (RoomType::Monster, 0.1),
    (RoomType::Elite, -1.0),
    (RoomType::Treasure, 0.02),
    (RoomType::Shop, 0.03),
];

/// `UnknownMapPointOdds` (`Odds/UnknownMapPointOdds.cs`): the `?` room roll. It draws on the
/// run's `UnknownMapPoint` stream.
#[derive(Clone, Copy, Debug)]
pub struct UnknownMapPointOdds {
    /// `_nonEventOdds` in dictionary order: Monster, Elite, Treasure, Shop.
    pub odds: [(RoomType, f32); 4],
    /// `_baseOdds` in the same order (`SetBaseOdds`).
    pub base: [(RoomType, f32); 4],
    /// `Hook.ModifyOddsIncreaseForUnrolledRoomType` for Treasure: `DeadlyEvents` doubles it.
    pub treasure_increase: f32,
}

impl Default for UnknownMapPointOdds {
    fn default() -> Self {
        Self {
            odds: UNKNOWN_BASE_ODDS,
            base: UNKNOWN_BASE_ODDS,
            treasure_increase: 1.0,
        }
    }
}

impl UnknownMapPointOdds {
    /// The current odds of `room` (Monster, Elite, Treasure or Shop).
    #[must_use]
    pub fn get(&self, room: RoomType) -> Option<f32> {
        self.odds.iter().find(|(r, _)| *r == room).map(|&(_, v)| v)
    }

    /// The room types a `?` can become before the blacklist and the hooks: the four keys, then
    /// Event.
    #[must_use]
    pub fn room_types() -> Vec<RoomType> {
        UNKNOWN_BASE_ODDS
            .iter()
            .map(|&(room, _)| room)
            .chain([RoomType::Event])
            .collect()
    }

    /// `Roll(blacklist, runState)` on a non-first run with no
    /// `ModifyOddsIncreaseForUnrolledRoomType` listener (`Odds/UnknownMapPointOdds.cs:123-177`).
    /// `types` is the allowed set: [`UnknownMapPointOdds::room_types`] minus the blacklist,
    /// through `ModifyUnknownMapPointRoomTypes`. The default is Event if allowed, else the lowest
    /// allowed type in `RoomType` order. One `NextFloat()`; the first allowed non-negative entry
    /// whose running sum reaches it wins. Then the rolled type resets to its base and every other
    /// allowed type adds its base.
    ///
    /// # Panics
    /// If `types` is empty.
    pub fn roll(&mut self, types: &[RoomType], rng: &mut Rng) -> RoomType {
        let allowed = |room: RoomType| types.contains(&room);
        let mut rolled = if allowed(RoomType::Event) {
            RoomType::Event
        } else {
            *types
                .iter()
                .min_by_key(|t| t.enum_index())
                .expect("a ? can become some room")
        };
        let r = rng.next_float(1.0);
        let mut sum: f32 = 0.0;
        for &(room, value) in &self.odds {
            if allowed(room) && value >= 0.0 {
                sum += value;
                if r <= sum {
                    rolled = room;
                    break;
                }
            }
        }
        for (entry, &(room, base)) in self.odds.iter_mut().zip(&self.base) {
            if room == rolled {
                entry.1 = base;
            } else if allowed(room) {
                entry.1 += if room == RoomType::Treasure {
                    base * self.treasure_increase
                } else {
                    base
                };
            }
        }
        rolled
    }

    /// `DeadlyEvents.ModifyOddsIncreaseForUnrolledRoomType`: a treasure's odds grow twice as fast.
    pub fn double_treasure_increase(&mut self) {
        self.treasure_increase *= 2.0;
    }

    /// `ResetToBase` (between acts): every odds back to its base.
    pub const fn reset_to_base(&mut self) {
        self.odds = self.base;
    }

    /// `SetBaseOdds(room, base)`.
    pub fn set_base(&mut self, room: RoomType, base: f32) {
        for entry in &mut self.base {
            if entry.0 == room {
                entry.1 = base;
            }
        }
    }
}

/// The odds compare by their bits, so a run state is `Eq` (no odds value is ever NaN).
macro_rules! eq_by_bits {
    ($ty:ty, |$v:ident| $bits:expr) => {
        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                let bits = |$v: &Self| $bits;
                bits(self) == bits(other)
            }
        }

        impl Eq for $ty {}
    };
}

eq_by_bits!(CardRarityOdds, |o| o.offset.to_bits());
eq_by_bits!(PotionRewardOdds, |o| o.value.to_bits());
eq_by_bits!(UnknownMapPointOdds, |o| (
    o.odds.map(|(r, v)| (r, v.to_bits())),
    o.base.map(|(r, v)| (r, v.to_bits())),
    o.treasure_increase.to_bits(),
));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_to_decimal_keeps_seven_digits() {
        assert_eq!(decimal_from_f32(0.1), Decimal::new(1, 1));
        assert_eq!(decimal_from_f32(-2.5), Decimal::new(-25, 1));
        assert_eq!(decimal_from_f32(1.234_567_8), Decimal::new(1_234_568, 6));
        assert_eq!(decimal_from_f32(14.999_999), Decimal::from(15));
        assert_eq!(decimal_from_f32(-14.123_456), Decimal::new(-1_412_346, 5));
        assert_eq!(decimal_from_f32(0.999_999_94), Decimal::new(9_999_999, 7));
        // Rounding to 7 digits can carry into the integer part, changing an `(int)` cast.
        assert_eq!(decimal_from_f32(1.999_999_9), Decimal::from(2));
        assert_eq!(decimal_from_f32(0.0), Decimal::ZERO);
    }

    #[test]
    fn pity_grows_in_f32() {
        let mut odds = CardRarityOdds::new(0);
        let mut rng = Rng::new(7);
        for _ in 0..12 {
            if odds.roll(CardRarityOddsType::RegularEncounter, &mut rng) == CardRarity::Rare {
                assert_eq!(odds.offset, -0.05);
            }
        }
        // Three non-rare rolls: the referee prints -0.02 (T6S0, step 28).
        let mut odds = CardRarityOdds::new(0);
        for _ in 0..3 {
            odds.offset = (odds.offset + RARITY_GROWTH).min(MAX_RARITY_OFFSET);
        }
        assert_eq!(odds.offset.to_string(), "-0.02");
    }

    #[test]
    fn unknown_odds_accumulate_like_the_referee() {
        // T6S0: two `?` rooms (Event, then Treasure) leave shop at 0.089999996.
        let mut odds = UnknownMapPointOdds::default();
        for (room, base) in UNKNOWN_BASE_ODDS {
            if room != RoomType::Event {
                odds.odds
                    .iter_mut()
                    .find(|(r, _)| *r == room)
                    .expect("key")
                    .1 += base;
            }
        }
        let shop = odds.get(RoomType::Shop).expect("shop") + 0.03;
        assert_eq!(shop.to_string(), "0.089999996");
    }
}

/// `CombatRoom.GoldProportion` (a C# `float`, 1 by default): a monster room's share of its gold.
///
/// Set from `EncounterModel.CalculateGoldProportion` at combat end. Kept as the `f32`'s bits so
/// the combat state stays `Eq` and `Hash`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GoldProportion(u32);

impl GoldProportion {
    /// `1f`: no monster escaped.
    pub const ONE: Self = Self::new(1.0);
    /// `0.5f` (`GremlinMercNormal`: the fat gremlin fled with no gold stolen).
    pub const HALF: Self = Self::new(0.5);
    /// `0f`: no gold reward at all.
    pub const ZERO: Self = Self::new(0.0);

    const fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    /// The `float` value.
    #[must_use]
    pub const fn value(self) -> f32 {
        f32::from_bits(self.0)
    }

    /// The default `CalculateGoldProportion` (`EncounterModel.cs:368-371`):
    /// `1f - (float)escaped / (float)spawned`, `spawned` being the distinct monster models
    /// spawned this combat (`SpawnedEnemies`).
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_precision_loss,
        reason = "the C# `(float)int` casts; creature counts are tiny"
    )]
    pub fn from_escaped(escaped: usize, spawned: usize) -> Self {
        Self::new(1.0 - escaped as f32 / spawned as f32)
    }

    /// `GoldProportion > 0f` (`RewardsSet.cs:225`): whether the room gives gold at all.
    #[must_use]
    pub fn gives_gold(self) -> bool {
        self.value() > 0.0
    }

    /// `(int)Math.Round((float)gold * GoldProportion)` (`RewardsSet.cs:227`): the `float` product,
    /// widened to `double` for `Math.Round` (half to even), then truncated.
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "the C# casts; gold amounts are small"
    )]
    pub fn scale(self, gold: i32) -> i32 {
        f64::from(gold as f32 * self.value()).round_ties_even() as i32
    }
}

#[cfg(test)]
mod upgrade_tests {
    use super::*;

    fn roll(seed: u64, act_index: usize, ascension: u8, rarity: CardRarity) -> (f32, bool) {
        let mut rng = Rng::new(seed);
        let f = rng.clone().next_float(1.0);
        let upgraded = roll_for_upgrade(
            UpgradeRoll {
                base_chance: REWARD_UPGRADE_BASE,
                act_index,
                ascension,
                rarity,
                upgradable: true,
            },
            &mut rng,
        );
        assert_eq!(rng.counter(), 1, "one draw");
        (f, upgraded)
    }

    /// `CardFactory.cs:288-305`: non-Rare odds are `actIndex × 0.25m` (`0.125m` from
    /// `Scarcity`, A7) on top of the base; Rare cards keep the base.
    #[test]
    fn upgrade_odds_grow_with_the_act() {
        let mut per_act = [0_u32; 3];
        for seed in 0..4000 {
            for (act, count) in per_act.iter_mut().enumerate() {
                let threshold = [0.0, 0.25, 0.5][act];
                let (f, upgraded) = roll(seed, act, 0, CardRarity::Common);
                assert_eq!(upgraded, decimal_from_f32(f) <= decimal_from_f32(threshold));
                *count += u32::from(upgraded);
                let scarce = [0.0, 0.125, 0.25][act];
                let (f, upgraded) = roll(seed, act, 7, CardRarity::Uncommon);
                assert_eq!(upgraded, decimal_from_f32(f) <= decimal_from_f32(scarce));
                assert!(!roll(seed, act, 0, CardRarity::Rare).1);
            }
        }
        assert_eq!(per_act[0], 0);
        assert!((900..1100).contains(&per_act[1]), "{per_act:?}");
        assert!((1900..2100).contains(&per_act[2]), "{per_act:?}");
    }

    #[test]
    fn upgrade_draws_even_when_it_cannot_upgrade() {
        for (base_chance, upgradable) in
            [(REWARD_UPGRADE_BASE, false), (MERCHANT_UPGRADE_BASE, true)]
        {
            let mut rng = Rng::new(3);
            let roll = UpgradeRoll {
                base_chance,
                act_index: 2,
                ascension: 0,
                rarity: CardRarity::Common,
                upgradable,
            };
            assert!(!roll_for_upgrade(roll, &mut rng));
            assert_eq!(rng.counter(), 1);
        }
    }

    /// The roll is compared as `(decimal)float`: the float just above 0.25 rounds to `0.25m`.
    #[test]
    fn upgrade_threshold_is_compared_in_decimal() {
        let above = f32::from_bits(0.25_f32.to_bits() + 1);
        assert_eq!(decimal_from_f32(above), Decimal::new(25, 2));
    }
}

#[cfg(test)]
mod gold_proportion_tests {
    use super::GoldProportion;

    #[test]
    fn default_proportion_counts_distinct_spawned_models() {
        // `1f - 1f / 2f`, `1f - 1f / 3f`.
        assert_eq!(GoldProportion::from_escaped(1, 2), GoldProportion::HALF);
        assert_eq!(GoldProportion::from_escaped(0, 3), GoldProportion::ONE);
        assert_eq!(GoldProportion::from_escaped(1, 1), GoldProportion::ZERO);
        assert!(!GoldProportion::ZERO.gives_gold());
        let third = GoldProportion::from_escaped(1, 3);
        assert_eq!(third.value(), 1.0 - 1.0 / 3.0);
        // 10 * 0.6666667f = 6.666667 -> 7; 20 * .. = 13.333334 -> 13.
        assert_eq!((third.scale(10), third.scale(20)), (7, 13));
    }

    #[test]
    fn scaling_rounds_half_to_even() {
        // 5 * 0.5 = 2.5 -> 2, 7 * 0.5 = 3.5 -> 4 (`Math.Round` is banker's rounding).
        assert_eq!(GoldProportion::HALF.scale(5), 2);
        assert_eq!(GoldProportion::HALF.scale(7), 4);
        assert_eq!(GoldProportion::ONE.scale(17), 17);
    }
}
