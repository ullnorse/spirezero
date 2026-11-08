//! Engine models to the referee's ids. The referee prints `ModelId.Entry`, which every kind's
//! generated registry derives (`id`), so there is no table to keep in sync here.

use sts2_core::combat::Encounter;
use sts2_core::content::cards::CardKind;
use sts2_core::content::monsters::MonsterKind;
use sts2_core::content::potions::PotionKind;
use sts2_core::content::powers::PowerKind;
use sts2_core::content::relics::RelicKind;

/// The referee id of a card.
#[must_use]
pub const fn card(kind: CardKind) -> &'static str {
    kind.id()
}

/// A card's `CardRarity` as its enum value ([`CardKind::rarity`]). The referee's selector lists
/// draw-pile options ordered by rarity, then id (`CardSelectCmd.cs:489-494`).
#[must_use]
pub fn card_rarity(kind: CardKind) -> u8 {
    kind.rarity() as u8
}

/// The card with referee id `id`.
#[must_use]
pub fn card_from_id(id: &str) -> Option<CardKind> {
    CardKind::from_id(id)
}

/// The referee id of a relic.
#[must_use]
pub const fn relic(kind: RelicKind) -> &'static str {
    kind.id()
}

/// The relic with referee id `id`.
#[must_use]
pub fn relic_from_id(id: &str) -> Option<RelicKind> {
    RelicKind::from_id(id)
}

/// The referee id of a potion.
#[must_use]
pub const fn potion(kind: PotionKind) -> &'static str {
    kind.id()
}

/// The potion with referee id `id`.
#[must_use]
pub fn potion_from_id(id: &str) -> Option<PotionKind> {
    PotionKind::from_id(id)
}

/// The referee id of a power.
#[must_use]
pub const fn power(kind: PowerKind) -> &'static str {
    kind.id()
}

/// The referee id (`ModelId.Entry`) of an encounter.
#[must_use]
pub const fn encounter(encounter: Encounter) -> &'static str {
    encounter.id()
}

/// The encounter with referee id `id`.
#[must_use]
pub fn encounter_from_id(id: &str) -> Option<Encounter> {
    Encounter::from_id(id)
}

/// The referee id of an enemy; `None` for a pet (Osty, which the trace reports under `osty`, and
/// Byrdpip, `PaelsLegion`).
#[must_use]
pub const fn enemy(kind: &MonsterKind) -> Option<&'static str> {
    match kind {
        MonsterKind::Osty(_) | MonsterKind::Byrdpip(_) | MonsterKind::PaelsLegion(_) => None,
        _ => Some(kind.id()),
    }
}
