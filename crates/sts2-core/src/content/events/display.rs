//! What an event's text shows: its `DynamicVars` (`CanonicalVars` and the values
//! `CalculateVars` and the option callbacks set). Display only.

use super::{EventData, EventKind, colossal_flower, trial, welcome_to_wongos};
use crate::content::monsters::{BattleFriendV1, BattleFriendV2, BattleFriendV3, MonsterKind};
use crate::content::tables::{self, ModelVar, VarText};
use crate::decimal::Decimal;
use crate::run::RunState;
use crate::run::events::EventRoom;

/// A `DynamicVar`'s value as an event's text shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EventVar {
    /// A number.
    Number(Decimal),
    /// A `StringVar`.
    Text(VarText),
}

impl EventKind {
    /// `CanonicalVars` (with `None` for a base the event computes).
    #[must_use]
    pub fn canonical_vars(self) -> &'static [ModelVar] {
        tables::event_vars(self.id())
    }
}

impl EventRoom {
    /// The current page's description key when it is not its options' page (`SetEventState`
    /// with another text): `Trial`'s case page is `TRIAL.trialFormat` around the case's story.
    #[must_use]
    pub const fn page_description(&self) -> Option<&'static str> {
        match self.data {
            EventData::Trial {
                case: Some(_),
                verdict: None,
            } if self.finished.is_none() => Some("TRIAL.trialFormat"),
            _ => None,
        }
    }

    /// The event's `DynamicVars` by name as its text shows them now: the canonical ones, with
    /// the values the event rolled or set (`EventData`) and the ones it reads from the run
    /// (`DenseVegetation`'s rest-site heal). A var with no known value is left out.
    #[must_use]
    pub fn vars(&self, s: &RunState) -> Vec<(&'static str, EventVar)> {
        let mut vars: Vec<(&'static str, Option<EventVar>)> = self
            .kind
            .canonical_vars()
            .iter()
            .map(|v| {
                let value = v
                    .value
                    .map(EventVar::Number)
                    .or_else(|| v.text.map(EventVar::Text));
                (v.name, value)
            })
            .collect();
        let mut set =
            |name: &'static str, value: EventVar| match vars.iter_mut().find(|(n, _)| *n == name) {
                Some(slot) => slot.1 = Some(value),
                None => vars.push((name, Some(value))),
            };
        let number = |n: Decimal| EventVar::Number(n);
        let text = EventVar::Text;
        match (&self.data, self.kind) {
            (EventData::Amount(points), EventKind::WelcomeToWongos) => {
                let points = points.to_int_truncating();
                for (name, value) in welcome_to_wongos::point_vars(points) {
                    set(name, number(value));
                }
            }
            (EventData::Amount(amount), kind) => {
                let name = match kind {
                    EventKind::AbyssalBaths => "Damage",
                    EventKind::SpiralingWhirlpool | EventKind::UnrestSite => "Heal",
                    EventKind::CrystalSphere => "UncoverFutureCost",
                    _ => "Gold",
                };
                set(name, number(*amount));
            }
            (
                EventData::JungleMaze {
                    solo_gold,
                    join_gold,
                },
                _,
            ) => {
                set("SoloGold", number(*solo_gold));
                set("JoinForcesGold", number(*join_gold));
            }
            (EventData::SunkenTreasury { small, large }, _) => {
                set("SmallChestGold", number(*small));
                set("LargeChestGold", number(*large));
            }
            (EventData::TabletOfTruth { cost, .. }, _) => {
                set("DecipherMaxHpLoss", number(Decimal::from(*cost)));
            }
            (EventData::SlipperyBridge { hold_ons, card, .. }, _) => {
                // `CurrentHpLoss => 3 + NumberOfHoldOns`.
                set("HpLoss", number(Decimal::from(3 + hold_ons)));
                if let Some(c) = s.deck.get(*card) {
                    set("RandomCard", EventVar::Text(VarText::Card(c.id)));
                }
            }
            (EventData::EndlessConveyor { last, current, .. }, _) => {
                set("CurrentDishTitle", EventVar::Text(VarText::Dish(current)));
                if !last.is_empty() {
                    set("LastDishTitle", EventVar::Text(VarText::Dish(last)));
                }
            }
            (EventData::Trial { case, verdict }, _) => {
                if let Some(key) = case.and_then(trial::story_key) {
                    set("TrialStory", text(VarText::Event(key)));
                }
                if let Some(key) = verdict.and_then(trial::result_key) {
                    set("TrialResult", text(VarText::Event(key)));
                }
            }
            (EventData::Relic(relic), EventKind::WelcomeToWongos) => {
                set("RandomRelic", EventVar::Text(VarText::Relic(relic)));
            }
            (EventData::RelicTrader { owned, new }, _) => {
                let rows = [
                    ("TopRelicOwned", "TopRelicNew"),
                    ("MiddleRelicOwned", "MiddleRelicNew"),
                    ("BottomRelicOwned", "BottomRelicNew"),
                ];
                for ((o, n), (&i, &relic)) in rows.into_iter().zip(owned.iter().zip(new)) {
                    if let Some(r) = s.relics.get(i) {
                        set(o, EventVar::Text(VarText::Relic(r.kind.id())));
                    }
                    set(n, EventVar::Text(VarText::Relic(relic)));
                }
            }
            _ => {}
        }
        for (name, value) in self.read_vars(s) {
            set(name, value);
        }
        vars.into_iter()
            // `Trial.CalculateVars` rolls `EntrantNumber` on `Rng.Chaotic`, which the engine
            // cannot reproduce; its canonical -1 is never shown.
            .filter(|&(name, _)| !(self.kind == EventKind::Trial && name == "EntrantNumber"))
            .filter_map(|(name, value)| Some((name, value?)))
            .collect()
    }

    /// The vars an event reads from the run or fixed tables rather than its own fields.
    fn read_vars(&self, s: &RunState) -> Vec<(&'static str, EventVar)> {
        let number = |n: i32| EventVar::Number(Decimal::from(n));
        let text = EventVar::Text;
        match self.kind {
            // `CalculateVars`: `HealRestSiteOption.GetHealAmount(Owner)`.
            EventKind::DenseVegetation => {
                vec![("Heal", EventVar::Number(s.rest_site_heal_amount()))]
            }
            // `GenerateInitialOptions`: each friend's `MinInitialHp` (`ScaleHpForMultiplayer`
            // keeps it for one player).
            EventKind::BattlewornDummy => [
                ("Setting1Hp", MonsterKind::BattleFriendV1(BattleFriendV1)),
                ("Setting2Hp", MonsterKind::BattleFriendV2(BattleFriendV2)),
                ("Setting3Hp", MonsterKind::BattleFriendV3(BattleFriendV3)),
            ]
            .into_iter()
            .map(|(name, friend)| (name, number(friend.min_initial_hp(s.ascension))))
            .collect(),
            // `GoldVar(_prizeKeys[i], _prizeCosts[i])`.
            EventKind::ColossalFlower => ["Prize1", "Prize2", "Prize3"]
                .into_iter()
                .zip(colossal_flower::PRIZE_COSTS)
                .map(|(name, cost)| (name, number(cost)))
                .collect(),
            // `DrinkRandomPotion`: the title of the potion `LIFT` drinks (its belt slot).
            EventKind::StoneOfAllTime => self
                .options
                .iter()
                .find_map(|o| o.action.filter(|a| a.tag == "LIFT"))
                .and_then(|a| s.potions.get(a.arg).copied().flatten())
                .map(|id| ("DrinkRandomPotion", text(VarText::Potion(id))))
                .into_iter()
                .collect(),
            // `StringVar`s of each tea's `DynamicDescription`.
            EventKind::TeaMaster => [
                ("BoneTeaDescription", "BONE_TEA"),
                ("EmberTeaDescription", "EMBER_TEA"),
                ("TeaOfDiscourtesyDescription", "TEA_OF_DISCOURTESY"),
            ]
            .into_iter()
            .map(|(name, relic)| (name, text(VarText::RelicDescription(relic))))
            .collect(),
            // The character's `aromaPrinciple` (`MaintainControl`).
            EventKind::AromaOfChaos => {
                vec![("AromaPrinciple", text(VarText::Character("aromaPrinciple")))]
            }
            // The character's `goldMonologue` (`SecondChest`).
            EventKind::SunkenTreasury => {
                vec![("Monologue", text(VarText::Character("goldMonologue")))]
            }
            _ => Vec::new(),
        }
    }
}
