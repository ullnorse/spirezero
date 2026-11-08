//! A plain-text form of state that has to survive a file (the RL start corpus): the RNG
//! streams ([`Rng`], [`RunRngSet`], [`PlayerRngSet`]) and the relics' fields ([`RelicData`]).
//!
//! A value is written as whitespace-separated tokens and read back exactly ([`to_text`],
//! [`from_text`]): integers in decimal, `bool` as `0`/`1`, `Option` as `_` or `+` and the
//! value, a fixed-size array as its items, a struct as its fields in declaration order, a
//! string with `\` escapes (`\\`, `\_` for a space, `\n`, `\t`, and `\e` alone for the empty
//! string), content kinds as their `ModelId.Entry`. The form is not self-describing beyond
//! that; the type being read decides what comes next.
//!
//! [`RelicData`]: crate::content::relics::RelicData

use std::fmt;

use crate::combat::CardId;
use crate::content::cards::CardKind;
use crate::content::cards::pools;
use crate::content::characters::Character;
use crate::content::enchantments::{Enchantment, EnchantmentKind, EnchantmentStatus};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicKind;
use crate::rng::{PlayerRngSet, PlayerRngType, Rng, RngState, RunRngSet, RunRngType};
use crate::run::RunCard;
use crate::run::cards::COLORLESS_CARD_POOL;
use crate::run::map::MapCoord;

/// Why a text did not read back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextError(pub String);

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TextError {}

/// A type with a text form.
pub trait Text: Sized {
    /// Appends the value's tokens.
    fn write(&self, w: &mut Writer);
    /// Reads a value from the next tokens.
    ///
    /// # Errors
    /// If the tokens are not a value of this type.
    fn read(r: &mut Reader<'_>) -> Result<Self, TextError>;
}

/// Tokens being written, separated by single spaces.
#[derive(Debug, Default)]
pub struct Writer(String);

impl Writer {
    /// Appends one token (no whitespace in it).
    pub fn token(&mut self, token: impl fmt::Display) {
        use fmt::Write as _;
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        write!(self.0, "{token}").expect("writing to a String does not fail");
    }
}

/// Tokens being read.
#[derive(Debug)]
pub struct Reader<'a>(std::str::SplitAsciiWhitespace<'a>);

impl Reader<'_> {
    /// The next token.
    ///
    /// # Errors
    /// At the end of the text.
    pub fn token(&mut self) -> Result<&str, TextError> {
        self.0
            .next()
            .ok_or_else(|| TextError("the text ends early".to_owned()))
    }

    /// The next token, parsed.
    ///
    /// # Errors
    /// At the end of the text, or if the token does not parse.
    pub fn parse<T: std::str::FromStr>(&mut self) -> Result<T, TextError> {
        let token = self.token()?;
        token
            .parse()
            .map_err(|_| TextError(format!("{token:?} is not a {}", std::any::type_name::<T>())))
    }
}

/// The text form of `value`.
#[must_use]
pub fn to_text<T: Text>(value: &T) -> String {
    let mut w = Writer::default();
    value.write(&mut w);
    w.0
}

/// A value from its whole text form.
///
/// # Errors
/// If `text` is not a value of `T`, or has tokens after it.
pub fn from_text<T: Text>(text: &str) -> Result<T, TextError> {
    let mut r = Reader(text.split_ascii_whitespace());
    let value = T::read(&mut r)?;
    r.0.next().map_or(Ok(value), |extra| {
        Err(TextError(format!("unexpected {extra:?} after the value")))
    })
}

macro_rules! text_by_parse {
    ($($t:ty),*) => {$(
        impl Text for $t {
            fn write(&self, w: &mut Writer) {
                w.token(self);
            }

            fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
                r.parse()
            }
        }
    )*};
}

text_by_parse!(u8, u16, u32, u64, usize, i32, i64);

impl Text for bool {
    fn write(&self, w: &mut Writer) {
        w.token(u8::from(*self));
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        match r.token()? {
            "0" => Ok(false),
            "1" => Ok(true),
            other => Err(TextError(format!("{other:?} is not a bool (0 or 1)"))),
        }
    }
}

impl<T: Text> Text for Option<T> {
    fn write(&self, w: &mut Writer) {
        match self {
            None => w.token('_'),
            Some(value) => {
                w.token('+');
                value.write(w);
            }
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        match r.token()? {
            "_" => Ok(None),
            "+" => T::read(r).map(Some),
            other => Err(TextError(format!("{other:?} is not an option (_ or +)"))),
        }
    }
}

impl<T: Text, const N: usize> Text for [T; N] {
    fn write(&self, w: &mut Writer) {
        for item in self {
            item.write(w);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let items = (0..N).map(|_| T::read(r)).collect::<Result<Vec<_>, _>>()?;
        Ok(items
            .try_into()
            .unwrap_or_else(|_| unreachable!("N items were read")))
    }
}

impl<A: Text, B: Text> Text for (A, B) {
    fn write(&self, w: &mut Writer) {
        self.0.write(w);
        self.1.write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        Ok((A::read(r)?, B::read(r)?))
    }
}

impl Text for String {
    fn write(&self, w: &mut Writer) {
        use fmt::Write as _;
        if self.is_empty() {
            w.token("\\e");
            return;
        }
        let mut token = Self::with_capacity(self.len());
        for c in self.chars() {
            match c {
                '\\' => token.push_str("\\\\"),
                ' ' => token.push_str("\\_"),
                '\n' => token.push_str("\\n"),
                '\t' => token.push_str("\\t"),
                c if c.is_ascii_whitespace() => {
                    write!(token, "\\u{:02x}", u32::from(c))
                        .expect("writing to a String does not fail");
                }
                c => token.push(c),
            }
        }
        w.token(token);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let token = r.token()?;
        if token == "\\e" {
            return Ok(Self::new());
        }
        let bad = || TextError(format!("{token:?} is not an escaped string"));
        let mut out = Self::with_capacity(token.len());
        let mut chars = token.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next().ok_or_else(bad)? {
                '\\' => out.push('\\'),
                '_' => out.push(' '),
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'u' => {
                    let hex: Self = chars.by_ref().take(2).collect();
                    let code = u8::from_str_radix(&hex, 16).map_err(|_| bad())?;
                    out.push(char::from(code));
                }
                _ => return Err(bad()),
            }
        }
        Ok(out)
    }
}

/// A model id the engine knows as a `'static` string: a ported card or relic, a card of a
/// pool, or an upgraded starter relic.
fn static_id(id: &str) -> Option<&'static str> {
    CardKind::from_id(id)
        .map(CardKind::id)
        .or_else(|| RelicKind::from_id(id).map(RelicKind::id))
        .or_else(|| Character::pool_card(id).map(|(_, card)| card.id))
        .or_else(|| {
            COLORLESS_CARD_POOL
                .iter()
                .map(|card| card.id)
                .chain(pools::status().into_iter().map(|card| card.id))
                .chain(pools::curse().into_iter().map(|card| card.id))
                .find(|&known| known == id)
        })
        .or_else(|| crate::content::relics::touch_of_orobas::static_relic_id(id))
}

/// A model id (`ModelId.Entry`); it reads back only if the engine knows the id
/// ([`static_id`]).
impl Text for &'static str {
    fn write(&self, w: &mut Writer) {
        w.token(self);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let id = r.token()?;
        static_id(id).ok_or_else(|| TextError(format!("unknown model id {id:?}")))
    }
}

macro_rules! text_by_id {
    ($($t:ty),*) => {$(
        impl Text for $t {
            fn write(&self, w: &mut Writer) {
                w.token(self.id());
            }

            fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
                let id = r.token()?;
                <$t>::from_id(id).ok_or_else(|| {
                    TextError(format!("unknown {} {id:?}", stringify!($t)))
                })
            }
        }
    )*};
}

text_by_id!(CardKind, EnchantmentKind, PowerKind, RelicKind);

impl Text for Character {
    fn write(&self, w: &mut Writer) {
        w.token(self.name());
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let name = r.token()?;
        Self::from_name(name).ok_or_else(|| TextError(format!("unknown character {name:?}")))
    }
}

impl Text for CardId {
    fn write(&self, w: &mut Writer) {
        self.0.write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        u32::read(r).map(Self)
    }
}

impl Text for MapCoord {
    fn write(&self, w: &mut Writer) {
        (self.col, self.row).write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let (col, row) = <(usize, usize)>::read(r)?;
        Ok(Self { col, row })
    }
}

impl Text for Enchantment {
    fn write(&self, w: &mut Writer) {
        let Self {
            kind,
            amount,
            status,
            extra_damage,
        } = self;
        kind.write(w);
        amount.write(w);
        (*status == EnchantmentStatus::Disabled).write(w);
        extra_damage.write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        Ok(Self {
            kind: Text::read(r)?,
            amount: Text::read(r)?,
            status: if bool::read(r)? {
                EnchantmentStatus::Disabled
            } else {
                EnchantmentStatus::Normal
            },
            extra_damage: Text::read(r)?,
        })
    }
}

impl Text for RunCard {
    fn write(&self, w: &mut Writer) {
        let Self {
            id,
            upgrade_level,
            enchantment,
            counter,
            calculated_level,
            ..
        } = self;
        id.write(w);
        upgrade_level.write(w);
        enchantment.write(w);
        counter.write(w);
        calculated_level.write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let mut card = Self::new(Text::read(r)?);
        card.upgrade_level = Text::read(r)?;
        card.enchantment = Text::read(r)?;
        card.counter = Text::read(r)?;
        card.calculated_level = Text::read(r)?;
        Ok(card)
    }
}

/// The call counter, then the four state words.
impl Text for Rng {
    fn write(&self, w: &mut Writer) {
        let RngState { counter, s } = self.state();
        counter.write(w);
        s.write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        Ok(Self::from_state(RngState {
            counter: Text::read(r)?,
            s: Text::read(r)?,
        }))
    }
}

/// The seed string, the hashed seed, then each stream in [`RunRngType::ALL`] order.
impl Text for RunRngSet {
    fn write(&self, w: &mut Writer) {
        self.seed_string().to_owned().write(w);
        self.seed().write(w);
        for kind in RunRngType::ALL {
            self[kind].write(w);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let seed_string = String::read(r)?;
        let seed = u64::read(r)?;
        let rngs = <[Rng; RunRngType::ALL.len()]>::read(r)?;
        Ok(Self::from_parts(seed_string, seed, rngs))
    }
}

/// The player seed, then each stream in [`PlayerRngType::ALL`] order.
impl Text for PlayerRngSet {
    fn write(&self, w: &mut Writer) {
        self.seed().write(w);
        for kind in PlayerRngType::ALL {
            self[kind].write(w);
        }
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
        let seed = u64::read(r)?;
        let rngs = <[Rng; PlayerRngType::ALL.len()]>::read(r)?;
        Ok(Self::from_parts(seed, rngs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::relics::{OwnedRelic, RelicData};

    fn round_trip<T: Text + PartialEq + fmt::Debug>(value: &T) -> String {
        let text = to_text(value);
        assert_eq!(&from_text::<T>(&text).unwrap(), value, "{text}");
        text
    }

    #[test]
    fn rng_streams_round_trip() {
        let mut set = RunRngSet::new("ABC123");
        for (i, kind) in RunRngType::ALL.into_iter().enumerate() {
            for _ in 0..i * 3 {
                set[kind].next_int(100);
            }
        }
        round_trip(&set);
        round_trip(&RunRngSet::placeholder());
        let mut player = PlayerRngSet::for_slot(set.seed(), 0);
        player[PlayerRngType::Shops].next_double();
        round_trip(&player);
        round_trip(&RunRngSet::new("a b\\c\n\t\r\u{b}"));
    }

    #[test]
    fn strings_escape_whitespace() {
        assert_eq!(round_trip(&String::new()), "\\e");
        assert_eq!(round_trip(&"a b\\".to_owned()), "a\\_b\\\\");
        assert!(from_text::<String>("a\\x").is_err());
    }

    #[test]
    fn every_relic_data_round_trips() {
        for kind in RelicKind::ALL {
            let relic = OwnedRelic::from(kind);
            let text = round_trip(&relic.data);
            assert!(text.split(' ').next().is_some());
        }
    }

    #[test]
    fn relic_fields_round_trip() {
        use crate::content::relics::{
            archaic_tooth, fur_coat, paels_tooth, pen_nib, sea_glass, touch_of_orobas,
            unsettling_lamp,
        };
        let mut tooth = paels_tooth::Data::default();
        let mut bell = RunCard::new("CURSE_OF_THE_BELL");
        bell.upgrade_level = 1;
        bell.enchantment = Some(Enchantment {
            kind: EnchantmentKind::ALL[0],
            amount: 3,
            status: EnchantmentStatus::Disabled,
            extra_damage: 2,
        });
        bell.counter = 7;
        bell.calculated_level = Some(0);
        tooth.cards[1] = Some(bell);
        let mut coat = fur_coat::Data::default();
        coat.cols[2] = 4;
        coat.len = 3;
        for data in [
            RelicData::None,
            RelicData::PaelsTooth(tooth),
            RelicData::FurCoat(coat),
            RelicData::PenNib(pen_nib::Data {
                attacks_played: 9,
                attack_to_double: Some(CardId(12)),
            }),
            RelicData::SeaGlass(sea_glass::Data {
                character: Some(Character::Regent),
            }),
            RelicData::ArchaicTooth(archaic_tooth::Data {
                starter_card: Some(RunCard::new("BASH")),
                ancient_card: None,
            }),
            RelicData::TouchOfOrobas(touch_of_orobas::Data {
                starter_relic: Some("BURNING_BLOOD"),
                upgraded_relic: Some("BLACK_BLOOD"),
            }),
            RelicData::UnsettlingLamp(unsettling_lamp::Data {
                triggering_card: Some(CardId(3)),
                doubled_temporary_stat: Some(PowerKind::ALL[1]),
                is_finished_triggering: true,
            }),
        ] {
            round_trip(&data);
        }
        assert!(from_text::<RelicData>("Nunchaku").is_err());
        assert!(from_text::<RelicData>("Nunchaku 1 2").is_err());
        assert!(from_text::<RelicData>("NoSuchRelic 1").is_err());
    }
}
