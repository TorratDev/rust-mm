//! Using items: the eat/use flow `0217371C` (actions `0x0C`, `0x0D` and the AI's `0x23`) and the
//! handler switch of `02146880` (`docs/re/items.md`, P2 and "P2 (#459)"). Orbs (action `0x31`,
//! `02191A7C`) run their move through the move path (`combat::Dungeon::use_orb`).
//!
//! The flow: pick the item, the can-use check (`021457B0`), consume it (`02141680`), then the
//! dispatcher: a food-class item (category 2) first adds a silent +5 belly, then the arm of the
//! item id runs. The message and wake-up steps (`0213F11C`, `02143C8C`, `0219037C`) and the
//! experience pass `02183BB4` are not modelled. Integer math only.
//!
//! The numbers the notes do not record (heal amounts, duration ranges, gummi tables) are
//! [`UseParams`], given by the caller as the item tables are.

use rescue_data::moves::Move;
use serde::{Deserialize, Serialize};

use crate::combat::CombatData;
use crate::command::{Blocked, Outcome};
use crate::items::{Item, STACKABLE_BELOW};
use crate::state::{Dungeon, MoveSlot};
use crate::status::{self, BURN, PARALYSIS};

// Item ids (Red's `ITEM_*` order, matched by value in `docs/re/items.md`, "P2 (#459)").
const HEAL_SEED: u8 = 0x35;
const WISH_STONE: u8 = 0x36;
const ORAN: u8 = 0x37;
const SITRUS: u8 = 0x38;
const RAWST: u8 = 0x3F;
const PECHA: u8 = 0x42;
const CHERI: u8 = 0x43;
const LIFE_SEED: u8 = 0x3E;
const DOOM_SEED: u8 = 0x3C;
const JOY_SEED: u8 = 0x4A;
const PLAIN_SEED: u8 = 0x46;
const SLEEP_SEED: u8 = 0x45;
const QUICK_SEED: u8 = 0x41;
const GINSENG: u8 = 0x49;
const MAX_ELIXIR: u8 = 0x4D;
const VITAMIN_FIRST: u8 = 0x4E;
const VITAMIN_LAST: u8 = 0x51;
const APPLE: u8 = 0x52;
const BIG_APPLE: u8 = 0x53;
const GRIMY_FOOD: u8 = 0x54;
const HUGE_APPLE: u8 = 0x55;
const GUMMI_FIRST: u8 = 0x56;
const GUMMI_LAST: u8 = 0x66;
const BANANA: u8 = 0x67;
const CHESTNUT: u8 = 0x68;
/// The parts, Music Box and Key: message-only arms (`0x77` to `0x7A`).
const PART_FIRST: u8 = 0x77;
const MUSIC_BOX: u8 = 0x7A;
const SPARE: u8 = 0xE9;
/// Item category 2: berry, seed, vitamin (`020082FC`), the class that adds the silent +5 belly.
const CATEGORY_FOOD: u8 = 2;
/// Item category 9: orbs. Category 5 is a TM and 10 a link box.
pub(crate) const CATEGORY_ORB: u8 = 9;
const CATEGORY_TM: u8 = 5;
const CATEGORY_LINK_BOX: u8 = 10;

/// The belly a food-class item adds before its arm (`0218C284(.., 5, 0, 0)`).
const FOOD_BELLY: i32 = 5;
/// What a vitamin adds to its stat (the 3 each arm loads), and the stat cap.
const VITAMIN_GAIN: u8 = 3;
/// The level cap of the Joy Seed.
const LEVEL_CAP: u8 = 100;
/// The cap of a maximum-HP rise.
// hypothesis: 999, as the game's HP field limit; the cap of `0218DF38` is not read.
const MAX_HP_CAP: i16 = 999;
/// The belly a team member starts with as its maximum, in thousandths.
pub(crate) const START_MAX_BELLY: i32 = 100_000;
/// The cap of a maximum-belly rise, in thousandths: `0218C284` clamps with `0200BA3C(.., 0200BA18(200))`.
const MAX_BELLY_CAP: i32 = 200_000;
/// The amount `0218C284` compares with `*DAT_ov_0001__0218c894`: it fills the belly and takes
/// the rise branch.
// hypothesis: the word is 999, the Huge Apple's amount; it is not read on a Ghidra copy.
const FILL_AMOUNT: i32 = 999;
/// The side of the gummi tables: 18 types by 18 gummi types (row stride `0x24` bytes).
const GUMMI_SIDE: usize = 18;

/// A heal and the maximum-HP rise that goes with it (`0218DF38(u, t, heal, maxHpRise, 1)`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealAmount {
    pub heal: i16,
    pub max_rise: i16,
}

/// The numbers the item handlers read from overlay literals and tables, which
/// `docs/re/items.md` does not record. The defaults are 0 and empty: a handler whose number is
/// missing does nothing with it (a 0 heal, a 0 IQ gain, no orb move).
// shortcut: one set for the run; the game reads them from the overlay literals (`021AA248`
// and neighbours) and the gummi tables at `02092590` and `02092308`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UseParams {
    /// Oran (`021AA248`, `021AA240`).
    pub oran: HealAmount,
    /// Sitrus (`021AA238`, `021AA230`).
    pub sitrus: HealAmount,
    /// Life Seed (`021AA22C`; the heal is 0 in the notes' reading).
    pub life: HealAmount,
    /// The Sleep Seed's duration range (`02146840`).
    pub sleep_turns: (i32, i32),
    /// The Quick Seed's duration range (`021466D4`).
    pub quick_turns: (i32, i32),
    /// Ginseng: the percent below which the roll gives +3 (`021AA200`).
    pub ginseng_percent: i32,
    /// Ginseng: the cap of a move's upgrade level (`0200B384`).
    pub ginseng_cap: u8,
    /// The gummi belly table (`DAT_02146074`): s16 words indexed `[eater type][gummi type]`, 18
    /// by 18; a missing entry is 0.
    pub gummi_belly: Vec<i16>,
    /// The gummi IQ table (`DAT_02146078`), the same shape.
    pub gummi_iq: Vec<i16>,
    /// The IQ cap a gummi clamps to.
    pub iq_cap: i16,
    /// The move of each orb, by item id (`02007670`, item table `+0x18`); 0 or past the end is
    /// no move.
    pub orb_moves: Vec<u16>,
}

impl UseParams {
    /// Empty parameters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            oran: HealAmount {
                heal: 0,
                max_rise: 0,
            },
            sitrus: HealAmount {
                heal: 0,
                max_rise: 0,
            },
            life: HealAmount {
                heal: 0,
                max_rise: 0,
            },
            sleep_turns: (0, 0),
            quick_turns: (0, 0),
            ginseng_percent: 0,
            ginseng_cap: 0,
            gummi_belly: Vec::new(),
            gummi_iq: Vec::new(),
            iq_cap: 0,
            orb_moves: Vec::new(),
        }
    }

    /// The move of orb `id`: `None` for 0 or an id past the table.
    #[must_use]
    pub fn orb_move(&self, id: u8) -> Option<u16> {
        self.orb_moves
            .get(usize::from(id))
            .copied()
            .filter(|&m| m != 0)
    }
}

/// What the dispatcher did with an item (for tests and the front end's message choice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseResult {
    /// An arm changed the state.
    Applied,
    /// The arm printed a message only (Plain Seed, the parts, a cure that found nothing to cure).
    MessageOnly,
    /// The id has no use effect: "nothing happens" (the Reviver Seed, TMs and link boxes, an id
    /// with no arm).
    NothingHappens,
    /// The arm is not ported: the item is consumed and nothing else happens. See the shortcut
    /// list on [`Dungeon::use_item_arm`].
    Unported,
}

impl Dungeon {
    /// The item id and result of the last use, if any.
    #[must_use]
    pub fn last_item_use(&self) -> Option<(u8, UseResult)> {
        self.items.last_use
    }

    /// The belly maximum of team member `member`, in thousandths.
    #[must_use]
    pub fn max_belly(&self, member: usize) -> i32 {
        self.items
            .max_belly
            .get(member)
            .copied()
            .unwrap_or(START_MAX_BELLY)
    }

    /// `02141680` for a bag slot: a stackable item (id below 9) with more than one loses one;
    /// else the slot is cleared and the bag compacted.
    pub(crate) fn consume_bag_slot(&mut self, slot: u8) {
        let bag = &mut self.items.bag;
        let Some(item) = bag.slots.get_mut(usize::from(slot)) else {
            return;
        };
        if item.id < STACKABLE_BELOW && item.qty > 1 {
            item.qty -= 1;
        } else {
            *item = Item::default();
            bag.compact();
        }
    }

    /// Actions `0x0C` and `0x0D` for the leader's bag slot `slot` (`0217371C`). Refusals change
    /// nothing and take no time: no item, a sticky item (`021457B0`). An orb goes through the move
    /// path (action `0x31`); a TM or link box is `NotModelled` (the teach-move screen is not
    /// read). Otherwise the item is consumed and its arm runs, in the leader's turn.
    // shortcut: the can-use check's second refusal (`info+0xCB == 1` with a food-type item,
    // `020073CC`) is not modelled: the sim keeps no `+0xCB` state and the food-type test is
    // unread. The message (`0213F11C`), `02143C8C`, the monster-house wake-up (`0219037C`) and the
    // experience and evolution pass `02183BB4` that ends the dispatcher are not modelled.
    pub(crate) fn use_item_command(&mut self, slot: u8, data: Option<&CombatData<'_>>) -> Outcome {
        if self.team.is_empty() {
            return Outcome::Blocked(Blocked::NoLeader);
        }
        let Some(item) = self
            .items
            .bag
            .slots
            .get(usize::from(slot))
            .copied()
            .filter(|i| i.exists())
        else {
            return Outcome::Blocked(Blocked::NoItem);
        };
        if item.sticky() {
            return Outcome::Blocked(Blocked::ItemSticky);
        }
        match self.items.tables.category(item.id) {
            CATEGORY_ORB => match data {
                Some(data) => self.use_orb(slot, item, None, data),
                None => Outcome::NotModelled,
            },
            CATEGORY_TM | CATEGORY_LINK_BOX => Outcome::NotModelled,
            _ if item.id == MAX_ELIXIR && data.is_none() => Outcome::NotModelled,
            _ => {
                let moves = data.map(|d| d.moves);
                self.take_leader_turn(data, |d| {
                    d.consume_bag_slot(slot);
                    d.items.last_use = Some((item.id, d.use_item_arm(0, item.id, moves)));
                });
                Outcome::Applied
            }
        }
    }

    /// Action `0x23`, the AI eats its held item (`0217371C` for a non-leader). The held slot is
    /// cleared (`02141680`) and the arm runs with the member as user and target. The monster's
    /// own turn is the caller's: no leader turn passes.
    // shortcut: the dispatcher's held-item auto-equip branch (non-leader user, category not 2,
    // gate `0218163C`, `02143BF8`) is not modelled: every item goes to its arm. The AI's choice
    // to eat is not ported (`docs/re/ai.md`), so nothing calls this yet.
    pub fn ai_eat(&mut self, member: usize, data: Option<&CombatData<'_>>) -> Outcome {
        let Some(held) = self.items.held.get(member).copied().filter(|i| i.exists()) else {
            return Outcome::Blocked(Blocked::NoItem);
        };
        if held.sticky() {
            return Outcome::Blocked(Blocked::ItemSticky);
        }
        if held.id == MAX_ELIXIR && data.is_none() {
            return Outcome::NotModelled;
        }
        self.items.held[member] = Item::default();
        let result = self.use_item_arm(member, held.id, data.map(|d| d.moves));
        self.items.last_use = Some((held.id, result));
        Outcome::Applied
    }

    /// `02146880` with `isThrown == 0`: the dispatcher for team member `member` using item `id`
    /// on itself.
    ///
    /// Shortcuts (arms that print "nothing happens" here and are listed for later slices; the item
    /// is still consumed): the thrown line ids 1 to 6 and rocks 7 and 8 (their use path calls the
    /// thrown handlers); Eyedrop `0x39`, Blinker `0x3B`, Allure `0x3D`, Totter `0x44`, Chesto
    /// `0x4B` and Stun `0x4C` (the sim has no blind, confusion or sleepless state beyond what
    /// [`status`] keeps); Hunger `0x40` (`0218C8B0`); Warp `0x47` (`0219BFE0` needs the floor
    /// grid scan); Blast `0x48` (a damage word read from `gDungeon+0x665`); the Key `0x7B`
    /// (`0215277C`); Grimy Food's arms 0, 1 and 4 (`0218A410`, `02189CBC`, `02189534`, whose bodies
    /// are unread: the roll is drawn, the arm does nothing, so the RNG after arms 0 and 1 differs
    /// from the game's one `0219CF28` draw); the first-dungeon level change of Joy and Doom does
    /// not recompute the stats (`02183994`, `02183644` need the level tables); the gummi message
    /// and the species test on `+0x40`; the Ginseng flag test (`0200B3E8`) and the second flag
    /// bit; the held-item auto-equip branch.
    pub(crate) fn use_item_arm(
        &mut self,
        member: usize,
        id: u8,
        moves: Option<&[Move]>,
    ) -> UseResult {
        let category = self.items.tables.category(id);
        if category == CATEGORY_FOOD {
            self.add_belly(member, FOOD_BELLY, 0);
        }
        if category == CATEGORY_TM || category == CATEGORY_LINK_BOX {
            return UseResult::NothingHappens;
        }
        match id {
            HEAL_SEED => {
                self.info_mut_of(member, status::cure_all);
                UseResult::Applied
            }
            ORAN => self.heal(member, self.items.tables.use_params.oran),
            SITRUS => self.heal(member, self.items.tables.use_params.sitrus),
            LIFE_SEED => self.heal(member, self.items.tables.use_params.life),
            RAWST => self.cure(member, &[BURN]),
            PECHA => self.cure(member, &[2, 3]),
            CHERI => self.cure(member, &[PARALYSIS]),
            SLEEP_SEED => {
                let pair = self.items.tables.use_params.sleep_turns;
                if let Some(m) = self.team.get_mut(member) {
                    status::sleep_with(&mut self.rng, &mut m.info, pair);
                }
                UseResult::Applied
            }
            QUICK_SEED => self.quick_seed(member),
            PLAIN_SEED | WISH_STONE | SPARE | PART_FIRST..=MUSIC_BOX => UseResult::MessageOnly,
            JOY_SEED => {
                self.info_mut_of(member, |i| {
                    i.level = i.level.saturating_add(1).min(LEVEL_CAP)
                });
                UseResult::Applied
            }
            DOOM_SEED => {
                self.info_mut_of(member, |i| i.level = i.level.saturating_sub(1).max(1));
                UseResult::Applied
            }
            GINSENG => self.ginseng(member),
            MAX_ELIXIR => self.max_elixir(member, moves),
            VITAMIN_FIRST..=VITAMIN_LAST => {
                let stat = usize::from(id - VITAMIN_FIRST);
                self.info_mut_of(member, |i| {
                    let cell = match stat {
                        0 => &mut i.attack[0],
                        1 => &mut i.attack[1],
                        2 => &mut i.defence[0],
                        _ => &mut i.defence[1],
                    };
                    *cell = cell.saturating_add(VITAMIN_GAIN);
                });
                UseResult::Applied
            }
            APPLE | BANANA => self.food(member, 50, 5),
            BIG_APPLE => self.food(member, 100, 10),
            HUGE_APPLE => self.food(member, 999, 10),
            CHESTNUT => self.food(member, 10, 0),
            GRIMY_FOOD => self.grimy_food(member),
            GUMMI_FIRST..=GUMMI_LAST => self.gummi(member, usize::from(id - GUMMI_FIRST)),
            0x39 | 0x3B | 0x3D | 0x44 | 0x4B | 0x4C | 0x40 | 0x47 | 0x48 | 0x7B | 1..=8 => {
                UseResult::Unported
            }
            _ => UseResult::NothingHappens,
        }
    }

    fn info_mut_of(&mut self, member: usize, f: impl FnOnce(&mut crate::state::EntityInfo)) {
        if let Some(m) = self.team.get_mut(member) {
            f(&mut m.info);
        }
    }

    /// `0218C284(user, target, amount, maxRise, 1)`: `amount` whole belly points. With a rise and
    /// a belly already at the maximum, or the fill amount, the maximum rises by `max_rise` (cap
    /// 200) and the belly is set to it; else the belly is the sum clamped to the maximum, and the
    /// fill amount sets it to the maximum.
    // hypothesis: the belly maximum (`+0x114` in Red) is kept per member in the item state,
    // starting at 100; `FILL_AMOUNT` is not read.
    // shortcut: the belly maximum resets to 100 on a restore, the message cases are not read, and
    // the refusal at the top of `0218C284` (`info+7 == 0` with `021432E8(t, 0x1B)`) is not
    // ported.
    fn add_belly(&mut self, member: usize, amount: i32, max_rise: i32) {
        let Some(max) = self.items.max_belly.get_mut(member) else {
            return;
        };
        let Some(m) = self.team.get_mut(member) else {
            return;
        };
        let fill = amount == FILL_AMOUNT;
        if max_rise != 0 && (m.info.belly >= *max || fill) {
            *max = max.saturating_add(max_rise * 1000).min(MAX_BELLY_CAP);
            m.info.belly = *max;
        } else {
            let sum = m.info.belly.saturating_add(amount.saturating_mul(1000));
            m.info.belly = if fill { *max } else { sum.min(*max) };
        }
    }

    fn food(&mut self, member: usize, amount: i32, max_rise: i32) -> UseResult {
        self.add_belly(member, amount, max_rise);
        UseResult::Applied
    }

    /// `0218DF38`: heals `heal`; at full HP, or with a heal of 0, the maximum rises instead.
    // `0218DF38` adds `maxRise` to both `+0x0E` and `+0x10` in the branch
    // `(hp == maxHp && maxRise > 0) || heal == 0`, clamps the maximum to `DAT_ov_0001__0218e17c`
    // and the HP to the maximum.
    // hypothesis: the cap value `DAT_0218e17c` is 999 (`MAX_HP_CAP`); it is not read.
    fn heal(&mut self, member: usize, a: HealAmount) -> UseResult {
        self.info_mut_of(member, |i| {
            if a.heal == 0 || i.hp >= i.max_hp {
                let new_max = i
                    .max_hp
                    .saturating_add(a.max_rise)
                    .clamp(i.max_hp, MAX_HP_CAP);
                let rise = new_max - i.max_hp;
                i.max_hp = new_max;
                i.hp = i.hp.saturating_add(rise).min(i.max_hp);
            } else {
                i.hp = i.hp.saturating_add(a.heal).min(i.max_hp);
            }
        });
        UseResult::Applied
    }

    /// A cure berry: `0218FD80` ends the class status, else a "nothing" message.
    fn cure(&mut self, member: usize, classes: &[u8]) -> UseResult {
        let cured = self
            .team
            .get_mut(member)
            .is_some_and(|m| status::cure_burn_class(&mut m.info, classes));
        if cured {
            UseResult::Applied
        } else {
            UseResult::MessageOnly
        }
    }

    /// `021466D4` -> `0218844C(.., 0, 1)`: one duration roll (`0219CF28`, flag 0) after the
    /// validity test, then, below speed stage 4, `+1` in the first free of the five speed-up
    /// counters.
    // The P2 note "no draw when the speed stage is already 4" is contradicted by the export: the
    // draw comes before the `02181E58(..) == 4` test, which only skips the counter write.
    fn quick_seed(&mut self, member: usize) -> UseResult {
        let pair = self.items.tables.use_params.quick_turns;
        let base = self.leader_speed;
        let Some(m) = self.team.get_mut(member) else {
            return UseResult::Unported;
        };
        let turns = status::status_turns(&mut self.rng, &m.info, pair, false);
        if status::speed_stage(&m.info, base) >= 4 {
            return UseResult::MessageOnly;
        }
        if let Some(c) = m.info.speed_up.iter_mut().find(|c| **c == 0) {
            *c = turns.saturating_add(1);
        }
        UseResult::Applied
    }

    /// `02146254`: `DungeonRandInt(100)` first, for every user; a non-leader (`info+7 == 0` tests
    /// `isTeamLeader`) stops there.
    /// Below the percent the gain is 3, else 1, on each held move up to the cap.
    // shortcut: the move test is the slot holding a move; the game's two flag bits and
    // `0200B3E8` are not decoded.
    fn ginseng(&mut self, member: usize) -> UseResult {
        let roll = self.rng.int(100);
        let p = &self.items.tables.use_params;
        let (percent, cap) = (p.ginseng_percent, p.ginseng_cap);
        let Some(m) = self.team.get_mut(member) else {
            return UseResult::Unported;
        };
        if !m.info.is_team_leader {
            return UseResult::MessageOnly;
        }
        let gain: u8 = if roll < percent { 3 } else { 1 };
        for slot in &mut m.info.moves {
            if slot.flags & MoveSlot::HOLDS_MOVE != 0 {
                slot.ginseng = slot.ginseng.saturating_add(gain).min(cap.max(slot.ginseng));
            }
        }
        UseResult::Applied
    }

    /// `0218B8C8`: each move below its base PP is restored. Needs the move table.
    fn max_elixir(&mut self, member: usize, moves: Option<&[Move]>) -> UseResult {
        let (Some(table), Some(m)) = (moves, self.team.get_mut(member)) else {
            return UseResult::Unported;
        };
        for slot in &mut m.info.moves {
            if slot.flags & MoveSlot::HOLDS_MOVE == 0 {
                continue;
            }
            if let Some(mv) = table.get(usize::from(slot.id)) {
                slot.pp = slot.pp.max(mv.base_pp);
            }
        }
        UseResult::Applied
    }

    /// `02145660`: belly 30, then `DungeonRandInt(5)`: arm 2 burns, arm 3 paralyses.
    fn grimy_food(&mut self, member: usize) -> UseResult {
        self.add_belly(member, 30, 0);
        let arm = self.rng.int(5);
        let Some(m) = self.team.get_mut(member) else {
            return UseResult::Unported;
        };
        match arm {
            2 => status::burn(&mut self.rng, &mut m.info),
            3 => status::paralyse(&mut self.rng, &mut m.info),
            _ => return UseResult::Unported,
        }
        UseResult::Applied
    }

    /// `02145EC8`: belly first (no maximum rise), then the IQ for a team member, clamped to 1
    /// and the cap. Both gains sum the table entries of the eater's two types at the gummi type.
    fn gummi(&mut self, member: usize, index: usize) -> UseResult {
        let p = &self.items.tables.use_params;
        let Some(m) = self.team.get(member) else {
            return UseResult::Unported;
        };
        let types = m.info.types;
        let col = index + 1;
        let sum = |table: &[i16]| -> i32 {
            types
                .iter()
                .map(|&t| {
                    let at = usize::from(t) * GUMMI_SIDE + col;
                    i32::from(table.get(at).copied().unwrap_or(0))
                })
                .sum()
        };
        let (belly, iq) = (sum(&p.gummi_belly), sum(&p.gummi_iq));
        let cap = p.iq_cap;
        self.add_belly(member, belly, 0);
        self.info_mut_of(member, |i| {
            if !i.not_team_member && cap > 0 {
                let v = i32::from(i.iq).saturating_add(iq).clamp(1, i32::from(cap));
                i.iq = v as i16;
            }
        });
        UseResult::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::floor::{Floor, HEIGHT, TERRAIN_FLOOR, WIDTH};
    use crate::items::{FLAG_STICKY, ItemTables};
    use crate::rng::DungeonRng;
    use crate::state::{EntityInfo, GameState, Monster};

    fn info(leader: bool) -> EntityInfo {
        EntityInfo {
            species: 1,
            apparent_species: 1,
            not_team_member: false,
            is_team_leader: leader,
            level: 5,
            iq: 10,
            hp: 20,
            max_hp: 30,
            attack: [10, 10],
            defence: [5, 5],
            offensive_stages: [10, 10],
            defensive_stages: [10, 10],
            accuracy_stage: 10,
            evasion_stage: 10,
            types: [1, 2],
            abilities: [0, 0],
            moves: [MoveSlot::default(); 4],
            speed_up: [0; 5],
            speed_down: [0; 5],
            direction: 0,
            belly: 50_000,
            belly_empty: false,
            regen_acc: 0,
            status: Default::default(),
            regen_speed: 0,
            exp: 0,
            exp_gained: 0,
        }
    }

    fn open_floor() -> Floor {
        let mut f = Floor::new();
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                f.tile_mut(x, y).unwrap().terrain = TERRAIN_FLOOR;
            }
        }
        f
    }

    /// An 18 by 18 gummi table whose row `t` holds `per_type[t]` (0 past the slice).
    fn gummi_table(per_type: &[i16]) -> Vec<i16> {
        (0..GUMMI_SIDE * GUMMI_SIDE)
            .map(|k| per_type.get(k / GUMMI_SIDE).copied().unwrap_or(0))
            .collect()
    }

    /// A run with the leader and an ally, and tables that class the seeds, berries, vitamins,
    /// apples and gummis as category 2.
    fn run() -> GameState {
        let mut g = GameState::new(7);
        let team = vec![
            Monster {
                pos: (5, 5),
                visible: true,
                info: info(true),
            },
            Monster {
                pos: (9, 9),
                visible: true,
                info: info(false),
            },
        ];
        g.enter_dungeon(open_floor(), team, 0, &[]).unwrap();
        g.set_speed_stage(crate::turn::MAX_SPEED_STAGE);
        let mut tables = ItemTables {
            categories: vec![0; 256],
            ..ItemTables::default()
        };
        for id in 0x35..=0x68 {
            tables.categories[id] = CATEGORY_FOOD;
        }
        tables.categories[0x69] = 6;
        tables.use_params = UseParams {
            oran: HealAmount {
                heal: 10,
                max_rise: 2,
            },
            sitrus: HealAmount {
                heal: 50,
                max_rise: 5,
            },
            life: HealAmount {
                heal: 0,
                max_rise: 4,
            },
            sleep_turns: (3, 7),
            quick_turns: (4, 6),
            ginseng_percent: 50,
            ginseng_cap: 99,
            gummi_belly: gummi_table(&[0, 4, 6, 1]),
            gummi_iq: gummi_table(&[0, 1, 2, 1]),
            iq_cap: 20,
            orb_moves: Vec::new(),
        };
        g.dungeon_mut().unwrap().items.tables = tables;
        g
    }

    fn d(g: &mut GameState) -> &mut Dungeon {
        g.dungeon_mut().unwrap()
    }

    /// Puts `id` in bag slot 0 and uses it.
    fn eat(g: &mut GameState, id: u8) -> Outcome {
        d(g).items.bag.slots[0] = Item::new(id, 1);
        g.step(crate::command::Command::UseItem { slot: 0 })
    }

    fn leader(g: &mut GameState) -> &mut EntityInfo {
        &mut d(g).team[0].info
    }

    fn last(g: &mut GameState) -> UseResult {
        d(g).last_item_use().unwrap().1
    }

    #[test]
    fn eating_consumes_the_item_takes_the_turn_and_adds_the_silent_belly() {
        let mut g = run();
        d(&mut g).items.bag.slots[1] = Item::new(0x37, 1);
        let turn = d(&mut g).turn();
        assert_eq!(eat(&mut g, PLAIN_SEED), Outcome::Applied);
        // The Plain Seed is message-only; the food class still added +5 belly.
        assert_eq!(last(&mut g), UseResult::MessageOnly);
        assert_eq!(leader(&mut g).belly, 55_000);
        assert!(d(&mut g).turn() > turn);
        // The slot is consumed and the bag compacted: the Oran moved up.
        assert_eq!(d(&mut g).bag().slots[0].id, 0x37);
        assert!(!d(&mut g).bag().slots[1].exists());
    }

    #[test]
    fn a_sticky_or_empty_slot_is_refused_with_no_time() {
        let mut g = run();
        let turn = d(&mut g).turn();
        assert_eq!(
            g.step(crate::command::Command::UseItem { slot: 3 }),
            Outcome::Blocked(Blocked::NoItem)
        );
        d(&mut g).items.bag.slots[0] = Item {
            flags: FLAG_EXISTS | FLAG_STICKY,
            qty: 1,
            id: ORAN,
        };
        assert_eq!(
            g.step(crate::command::Command::UseItem { slot: 0 }),
            Outcome::Blocked(Blocked::ItemSticky)
        );
        assert!(d(&mut g).bag().slots[0].exists());
        assert_eq!(d(&mut g).turn(), turn);
    }

    use crate::items::FLAG_EXISTS;

    #[test]
    fn berries_heal_and_raise_the_maximum_only_at_full_hp() {
        let mut g = run();
        eat(&mut g, ORAN);
        assert_eq!(leader(&mut g).hp, 30); // 20 + 10, capped at the maximum
        assert_eq!(leader(&mut g).max_hp, 30);
        // At full HP the Oran raises the maximum by 2 and the HP with it.
        eat(&mut g, ORAN);
        assert_eq!((leader(&mut g).hp, leader(&mut g).max_hp), (32, 32));
        // The Life Seed has no heal: the maximum rises by 4.
        eat(&mut g, LIFE_SEED);
        assert_eq!((leader(&mut g).hp, leader(&mut g).max_hp), (36, 36));
        // The Sitrus heals below the maximum.
        leader(&mut g).hp = 1;
        eat(&mut g, SITRUS);
        assert_eq!(leader(&mut g).hp, 36); // 1 + 50, capped at the maximum
    }

    #[test]
    fn cure_berries_end_only_their_class() {
        let mut g = run();
        leader(&mut g).status.burn_class = BURN;
        leader(&mut g).status.burn_turns = 100;
        eat(&mut g, PECHA);
        assert_eq!(leader(&mut g).status.burn_class, BURN);
        assert_eq!(last(&mut g), UseResult::MessageOnly);
        eat(&mut g, RAWST);
        assert_eq!(leader(&mut g).status.burn_class, 0);
        assert_eq!(last(&mut g), UseResult::Applied);
        leader(&mut g).status.burn_class = PARALYSIS;
        leader(&mut g).status.burn_turns = 100;
        eat(&mut g, RAWST);
        assert_eq!(leader(&mut g).status.burn_class, PARALYSIS);
        eat(&mut g, CHERI);
        assert_eq!(leader(&mut g).status.burn_class, 0);
    }

    #[test]
    fn the_heal_seed_clears_the_bad_statuses() {
        let mut g = run();
        {
            let s = &mut leader(&mut g).status;
            s.sleep = status::SLEEP;
            s.sleep_turns = 4;
            s.burn_class = BURN;
            s.cringe = status::CONFUSED;
            s.leech_seed = status::LEECH_SEED;
            s.sure_shot = status::WHIFFER;
        }
        d(&mut g).use_item_arm(0, HEAL_SEED, None);
        let s = leader(&mut g).status;
        assert_eq!(
            (s.sleep, s.burn_class, s.cringe, s.leech_seed, s.sure_shot),
            (0, 0, 0, 0, 0)
        );
    }

    #[test]
    fn vitamins_add_three_to_their_stat_and_clamp_at_255() {
        let mut g = run();
        for (id, i) in [(0x4E, 0), (0x4F, 1), (0x50, 2), (0x51, 3)] {
            eat(&mut g, id);
            let l = leader(&mut g);
            let all = [l.attack[0], l.attack[1], l.defence[0], l.defence[1]];
            let base = [10, 10, 5, 5];
            for k in 0..4 {
                assert_eq!(
                    all[k],
                    base[k] + if k == i { 3 } else { 0 },
                    "id {id:#x} stat {k}"
                );
            }
            let l = leader(&mut g);
            l.attack = [10, 10];
            l.defence = [5, 5];
        }
        leader(&mut g).attack[0] = 254;
        eat(&mut g, 0x4E);
        assert_eq!(leader(&mut g).attack[0], 255);
    }

    #[test]
    fn foods_add_belly_and_a_full_belly_raises_the_maximum() {
        let mut g = run();
        // 50 + 5 + 50 overflows a belly that was not full: it clamps, the maximum stays.
        eat(&mut g, APPLE);
        assert_eq!(d(&mut g).max_belly(0), 100_000);
        assert_eq!(leader(&mut g).belly, 100_000);
        // Now full: the Chestnut has no rise and changes nothing.
        eat(&mut g, CHESTNUT);
        assert_eq!(d(&mut g).max_belly(0), 100_000);
        assert_eq!(leader(&mut g).belly, 100_000);
        // Full again: the Apple raises the maximum by 5 and fills to it.
        leader(&mut g).belly = 100_000;
        eat(&mut g, APPLE);
        assert_eq!(d(&mut g).max_belly(0), 105_000);
        // (the turn that follows the meal burns 100 of belly)
        assert!((104_900..=105_000).contains(&leader(&mut g).belly));
        // The Big Apple on a belly that is not full does not raise the maximum.
        let mut g = run();
        leader(&mut g).belly = 10_000;
        eat(&mut g, BIG_APPLE);
        assert_eq!(leader(&mut g).belly, 100_000);
        assert_eq!(d(&mut g).max_belly(0), 100_000);
        // The fill amount takes the rise branch whatever the belly.
        let mut g = run();
        d(&mut g).use_item_arm(0, HUGE_APPLE, None);
        assert_eq!(d(&mut g).max_belly(0), 110_000);
        assert_eq!(leader(&mut g).belly, 110_000);
        // The rise stops at 200.
        let mut g = run();
        for _ in 0..12 {
            d(&mut g).use_item_arm(0, HUGE_APPLE, None);
        }
        assert_eq!(d(&mut g).max_belly(0), 200_000);
    }

    #[test]
    fn gummis_add_belly_then_iq_with_the_clamp() {
        let mut g = run();
        eat(&mut g, 0x60);
        assert_eq!(leader(&mut g).belly, 65_000);
        assert_eq!(leader(&mut g).iq, 13);
        leader(&mut g).iq = 19;
        eat(&mut g, 0x56);
        assert_eq!(leader(&mut g).iq, 20);
        // Another pair of types gets another gain: types [3, 3] give 1 + 1 for each.
        let mut g = run();
        leader(&mut g).types = [3, 3];
        eat(&mut g, 0x60);
        assert_eq!(leader(&mut g).belly, 50_000 + 5_000 + 2_000);
        assert_eq!(leader(&mut g).iq, 12);
    }

    #[test]
    fn the_sleep_seed_draws_before_the_gates() {
        let mut g = run();
        // Immune target: the draw still happens, so the rng differs from a no-draw item's.
        leader(&mut g).abilities = [0x36, 0];
        let mut twin = run();
        leader(&mut twin).abilities = [0x36, 0];
        eat(&mut g, SLEEP_SEED);
        eat(&mut twin, PLAIN_SEED);
        assert_eq!(leader(&mut g).status.sleep, 0);
        assert_ne!(d(&mut g).rng(), d(&mut twin).rng());
        // A vulnerable one sleeps for a turn count inside the range.
        let mut g = run();
        eat(&mut g, SLEEP_SEED);
        let s = leader(&mut g).status;
        assert_eq!(s.sleep, status::SLEEP);
        assert!((1..=7).contains(&s.sleep_turns));
    }

    #[test]
    fn ginseng_draws_first_and_the_percent_picks_three_or_one() {
        let mut g = run();
        leader(&mut g).moves[0] = MoveSlot {
            flags: MoveSlot::HOLDS_MOVE,
            flags2: 0,
            id: 1,
            pp: 5,
            ginseng: 0,
        };
        d(&mut g).items.tables.use_params.ginseng_percent = 100; // always +3
        eat(&mut g, GINSENG);
        assert_eq!(leader(&mut g).moves[0].ginseng, 3);
        assert_eq!(leader(&mut g).moves[1].ginseng, 0); // an empty slot is skipped
        d(&mut g).items.tables.use_params.ginseng_percent = 0; // always +1
        eat(&mut g, GINSENG);
        assert_eq!(leader(&mut g).moves[0].ginseng, 4);
        d(&mut g).items.tables.use_params.ginseng_cap = 4;
        eat(&mut g, GINSENG);
        assert_eq!(leader(&mut g).moves[0].ginseng, 4);
    }

    #[test]
    fn a_non_leader_member_gets_one_draw_and_no_ginseng() {
        let mut g = run();
        d(&mut g).team[1].info.moves[0] = MoveSlot {
            flags: MoveSlot::HOLDS_MOVE,
            flags2: 0,
            id: 1,
            pp: 5,
            ginseng: 0,
        };
        let mut probe = *d(&mut g).rng();
        probe.int(100);
        assert_eq!(
            d(&mut g).use_item_arm(1, GINSENG, None),
            UseResult::MessageOnly
        );
        assert_eq!(d(&mut g).team[1].info.moves[0].ginseng, 0);
        assert_eq!(d(&mut g).rng(), &probe);
    }

    #[test]
    fn grimy_food_draws_five_then_burns_or_paralyses() {
        let mut seen = [false; 5];
        for seed in 0..40u32 {
            let mut g = GameState::new(seed);
            let team = vec![Monster {
                pos: (5, 5),
                visible: true,
                info: info(true),
            }];
            g.enter_dungeon(open_floor(), team, 0, &[]).unwrap();
            leader(&mut g).types = [0, 0];
            d(&mut g).items.tables.categories = vec![0; 256];
            d(&mut g).rng = DungeonRng::new(seed);
            // The arm is the first draw after the leader's turn start: run it directly.
            let mut probe: DungeonRng = *d(&mut g).rng();
            let arm = probe.int(5);
            let result = d(&mut g).use_item_arm(0, GRIMY_FOOD, None);
            let l = &d(&mut g).team[0].info;
            assert_eq!(l.belly, 80_000);
            match arm {
                2 => assert_eq!(l.status.burn_class, BURN),
                3 => assert_eq!(l.status.burn_class, PARALYSIS),
                _ => assert_eq!(result, UseResult::Unported),
            }
            seen[usize::try_from(arm).unwrap()] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn joy_and_doom_change_the_level_within_bounds() {
        let mut g = run();
        eat(&mut g, JOY_SEED);
        assert_eq!(leader(&mut g).level, 6);
        leader(&mut g).level = 100;
        eat(&mut g, JOY_SEED);
        assert_eq!(leader(&mut g).level, 100);
        leader(&mut g).level = 1;
        eat(&mut g, DOOM_SEED);
        assert_eq!(leader(&mut g).level, 1);
        eat(&mut g, DOOM_SEED);
        assert_eq!(last(&mut g), UseResult::Applied);
    }

    #[test]
    fn unported_and_no_effect_arms_leave_the_state_but_consume_the_item() {
        let mut g = run();
        for (id, want) in [
            (0x3A, UseResult::NothingHappens), // Reviver Seed
            (0x47, UseResult::Unported),       // Warp
            (0x69, UseResult::NothingHappens),
            (0x30, UseResult::NothingHappens),
        ] {
            assert_eq!(eat(&mut g, id), Outcome::Applied);
            assert_eq!(last(&mut g), want, "id {id:#x}");
            assert!(!d(&mut g).bag().slots[0].exists());
        }
    }

    #[test]
    fn the_ai_eats_its_held_item_without_a_turn() {
        let mut g = run();
        let turn = d(&mut g).turn();
        assert_eq!(d(&mut g).ai_eat(1, None), Outcome::Blocked(Blocked::NoItem));
        d(&mut g).items.held[1] = Item::new(ORAN, 1);
        d(&mut g).team[1].info.hp = 5;
        assert_eq!(d(&mut g).ai_eat(1, None), Outcome::Applied);
        assert_eq!(d(&mut g).team[1].info.hp, 15);
        assert!(!d(&mut g).held_item(1).unwrap().exists());
        assert_eq!(d(&mut g).turn(), turn);
        d(&mut g).items.held[1] = Item {
            flags: FLAG_EXISTS | FLAG_STICKY,
            qty: 1,
            id: ORAN,
        };
        assert_eq!(
            d(&mut g).ai_eat(1, None),
            Outcome::Blocked(Blocked::ItemSticky)
        );
    }

    #[test]
    fn a_stack_loses_one_when_used() {
        let mut g = run();
        d(&mut g).items.bag.slots[0] = Item::new(0x03, 4);
        d(&mut g).consume_bag_slot(0);
        assert_eq!(d(&mut g).bag().slots[0].qty, 3);
        d(&mut g).items.bag.slots[0].qty = 1;
        d(&mut g).consume_bag_slot(0);
        assert!(!d(&mut g).bag().slots[0].exists());
    }

    #[test]
    fn the_quick_seed_takes_the_first_free_counter_after_one_draw() {
        let mut g = run();
        d(&mut g).leader_speed = 1;
        eat(&mut g, QUICK_SEED);
        let c = leader(&mut g).speed_up[0];
        assert!((5..=7).contains(&c), "{c}");
        assert_eq!(leader(&mut g).speed_up[1], 0);
        // At stage 4 the draw happens (before the stage test) and nothing is written.
        let mut g = run();
        d(&mut g).leader_speed = 4;
        let mut probe = *d(&mut g).rng();
        probe.range(4, 6);
        d(&mut g).use_item_arm(0, QUICK_SEED, None);
        assert_eq!(leader(&mut g).speed_up, [0; 5]);
        assert_eq!(d(&mut g).rng(), &probe);
    }

    #[test]
    fn the_arms_draw_in_the_documented_order() {
        // Ginseng: one DungeonRandInt(100) and nothing else, even for a non-leader.
        for non_leader in [false, true] {
            let mut g = run();
            leader(&mut g).is_team_leader = !non_leader;
            let mut probe = *d(&mut g).rng();
            probe.int(100);
            d(&mut g).use_item_arm(0, GINSENG, None);
            assert_eq!(d(&mut g).rng(), &probe);
        }
        // Sleep: one range roll, drawn even when Insomnia refuses.
        let mut g = run();
        leader(&mut g).abilities = [0x36, 0];
        let mut probe = *d(&mut g).rng();
        probe.range(3, 7);
        d(&mut g).use_item_arm(0, SLEEP_SEED, None);
        assert_eq!(d(&mut g).rng(), &probe);
        // Grimy Food: Int(5), then the burn's constant pair draws nothing.
        let mut g = run();
        let mut probe = *d(&mut g).rng();
        probe.int(5);
        d(&mut g).use_item_arm(0, GRIMY_FOOD, None);
        let arm = {
            let mut p = *d(&mut run()).rng();
            p.int(5)
        };
        if arm == 2 {
            assert_eq!(d(&mut g).rng(), &probe);
        }
        // The no-draw groups draw nothing.
        for id in [
            ORAN, SITRUS, LIFE_SEED, RAWST, HEAL_SEED, 0x4E, APPLE, 0x58, JOY_SEED,
        ] {
            let mut g = run();
            let before = *d(&mut g).rng();
            d(&mut g).use_item_arm(0, id, None);
            assert_eq!(d(&mut g).rng(), &before, "id {id:#x}");
        }
    }
}
