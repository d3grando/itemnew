// Item Spotlight: items that just arrived in your inventory are marked on their
// slot until you have seen them.
//
// Original implementation for Horizon (a horizon-mixins native mod). This file is
// both the standalone, unit-tested Cargo library and the injected
// `crate::game::itemspotlight` module. It has no Horizon dependency: `native.rs`
// turns the running game's inventory into an [`Inventory`] each tick and paints
// the [`Fill`]s this module asks for.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The mod's id: its row in Options > Mods and its settings folder
/// (`horizon-mods/.horizon-mod-data/itemspotlight/`).
pub const ID: &str = "itemspotlight";
pub const VERSION: &str = "0.1.0";
pub const NAME: &str = "Item Spotlight";
pub const DESCRIPTION: &str = "Marks items newly added to your inventory, in the hotbar and inventory screens, until you have seen them.";
/// The configuration, in the mod's settings folder: what the settings page writes.
pub const CONFIG_FILE: &str = "config.json";
/// The chat command (without its slash).
pub const COMMAND: &str = "itemspotlight";
pub const USAGE: &str = "Usage: /itemspotlight [status|clear|reload]";

/// The player's 36 main slots (0..9 the hotbar), then the off hand.
pub const MAIN_SLOTS: usize = 36;
pub const OFFHAND: usize = 36;
pub const SLOTS: usize = 37;
/// The off hand's slot id in the game's menus (`Game::slot_mut`'s 504).
pub const OFFHAND_SLOT_ID: usize = 504;
/// Ticks after joining a world or changing dimension during which arrivals
/// are not marked: the inventory itself arrives then (a server sends it
/// after login and respawn), and none of it is new.
pub const GRACE_TICKS: u64 = 60;
pub const TICKS_PER_SECOND: u64 = 20;
/// Longest `timeout` the file may ask for, in seconds (a day).
pub const MAX_TIMEOUT: u32 = 86_400;
/// One slot's side in GUI pixels.
pub const CELL: f32 = 16.0;
/// One cycle of the pulse, in milliseconds.
pub const PULSE_MILLIS: u64 = 1_200;

// ---------------------------------------------------------------- configuration

/// How a marked slot looks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Style {
    /// A soft glow in the slot, brightest at its edges.
    #[default]
    Glow,
    /// A thin frame around the slot.
    Frame,
    /// A small twinkling star in the slot's top left corner.
    Star,
    /// The glow and the star.
    GlowStar,
}
/// The mark's colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Colour {
    #[default]
    Gold,
    /// The colour of the item's name (its rarity: white, yellow, aqua, pink).
    Rarity,
    White,
    Aqua,
    Lime,
    Pink,
}
/// What takes a mark away (besides the item leaving its slot).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Clear {
    /// Pointing at the slot in a screen, or closing a screen the mark showed in.
    #[default]
    HoverOrClose,
    Hover,
    Close,
    /// Neither: only `timeout`, the Clear button or `/itemspotlight clear`.
    Never,
}
impl Clear {
    pub fn on_hover(self) -> bool {
        matches!(self, Self::HoverOrClose | Self::Hover)
    }
    pub fn on_close(self) -> bool {
        matches!(self, Self::HoverOrClose | Self::Close)
    }
}

/// `config.json`. Every key is optional; unknown keys are an error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct Config {
    pub enabled: bool,
    pub style: Style,
    pub color: Colour,
    /// Marks on the HUD's hotbar (and off hand slot).
    pub hotbar: bool,
    /// Marks in the player's own inventory screen and the creative screen.
    pub inventory: bool,
    /// Marks in the player slots of every other screen: chests, furnaces,
    /// crafting tables, workstations, villagers, mounts.
    pub containers: bool,
    pub clear: Clear,
    /// Seconds after which a mark goes by itself; 0 never.
    pub timeout: u32,
    /// The mark pulses (and the star twinkles).
    pub animate: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            style: Style::Glow,
            color: Colour::Gold,
            hotbar: true,
            inventory: true,
            containers: true,
            clear: Clear::HoverOrClose,
            timeout: 0,
            animate: true,
        }
    }
}
impl Config {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if config.timeout > MAX_TIMEOUT {
            return Err(format!("timeout must be 0..={MAX_TIMEOUT} seconds"));
        }
        Ok(config)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
    /// Whether marks show on this kind of screen.
    pub fn shows(&self, screen: ScreenKind) -> bool {
        self.enabled
            && match screen {
                ScreenKind::Hotbar => self.hotbar,
                ScreenKind::Inventory => self.inventory,
                ScreenKind::Container => self.containers,
            }
    }
}
/// Where a slot is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenKind {
    Hotbar,
    Inventory,
    Container,
}

// ---------------------------------------------------------------- tracking

/// One slot's contents as the tracker compares them: the item and its count.
/// Damage and components are left out, so a tool that wears down or a
/// stack whose name changes stays the same item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
}
impl Stack {
    pub fn new(item: &str, count: u32) -> Self {
        Self {
            item: item.to_owned(),
            count,
        }
    }
}
/// The player's slots ([`SLOTS`]: 36 main slots, then the off hand) and the
/// stack carried on the cursor in a screen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub slots: Vec<Option<Stack>>,
    pub carried: Option<Stack>,
}
impl Inventory {
    fn slot(&self, i: usize) -> Option<&Stack> {
        self.slots.get(i).and_then(Option::as_ref)
    }
    /// Each item's count over every slot and the cursor.
    fn totals(&self) -> HashMap<&str, u64> {
        let mut totals = HashMap::new();
        let stacks = self.slots.iter().take(SLOTS).flatten();
        for s in stacks.chain(self.carried.as_ref()) {
            *totals.entry(s.item.as_str()).or_default() += u64::from(s.count);
        }
        totals
    }
}
/// A marked slot: what arrived, when (game ticks), and whether a screen has
/// shown it (closing that screen then clears it, with [`Clear::on_close`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub item: String,
    pub since: u64,
    pub seen: bool,
}

/// Which slots are marked, from one tick's inventory to the next.
///
/// A slot is marked when, since the last tick, it gained items (it was empty,
/// or held fewer of the same item) **and** the player now has more of that
/// item in total, the cursor included. So items picked up, given, crafted
/// with a Shift click or Shift-clicked out of a chest are marked; items
/// moved around the inventory, or put down from the cursor, are not (the
/// player just handled them); a slot whose item turned into another one in
/// place (a bucket filled) is not either.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    last: Option<Inventory>,
    marks: Vec<Option<Mark>>,
    context: Option<String>,
    quiet_until: u64,
    screen_open: bool,
}
impl Tracker {
    pub const fn new() -> Self {
        Self {
            last: None,
            marks: Vec::new(),
            context: None,
            quiet_until: 0,
            screen_open: false,
        }
    }
    /// Forget everything: no world, or the mod is off.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
    /// Take every mark away (the inventory stays the baseline).
    pub fn clear_marks(&mut self) {
        self.marks.clear();
    }
    pub fn marked(&self, slot: usize) -> Option<&Mark> {
        self.marks.get(slot).and_then(Option::as_ref)
    }
    /// The marked slots and their items.
    pub fn marks(&self) -> Vec<(usize, String)> {
        let marks = self.marks.iter().enumerate();
        marks
            .filter_map(|(i, m)| m.as_ref().map(|m| (i, m.item.clone())))
            .collect()
    }
    pub fn count(&self) -> usize {
        self.marks.iter().flatten().count()
    }
    fn set(&mut self, slot: usize, mark: Option<Mark>) {
        if slot >= SLOTS {
            return;
        }
        if self.marks.len() < SLOTS {
            self.marks.resize(SLOTS, None);
        }
        self.marks[slot] = mark;
    }
    /// The pointer is over marked slot `slot` in a screen: true when that
    /// took the mark away.
    pub fn hover(&mut self, slot: usize, config: &Config) -> bool {
        if !config.clear.on_hover() || self.marked(slot).is_none() {
            return false;
        }
        self.set(slot, None);
        true
    }
    /// A screen showed marked slot `slot`.
    pub fn saw(&mut self, slot: usize) {
        if let Some(Some(mark)) = self.marks.get_mut(slot) {
            mark.seen = true;
        }
    }
    /// One tick: `inventory` as it is now, at game tick `now`, in world
    /// `context` (a dimension; a change starts a grace period), with a
    /// screen of player slots open or not.
    pub fn observe(
        &mut self,
        now: u64,
        mut inventory: Inventory,
        context: &str,
        screen_open: bool,
        config: &Config,
    ) {
        inventory.slots.resize(SLOTS, None);
        if self.context.as_deref() != Some(context) {
            self.context = Some(context.to_owned());
            self.quiet_until = now.saturating_add(GRACE_TICKS);
        }
        if !config.enabled {
            self.marks.clear();
        }
        // A mark goes with its item.
        for slot in 0..SLOTS {
            let item = inventory.slot(slot).map(|s| s.item.as_str());
            if self
                .marked(slot)
                .is_some_and(|m| Some(m.item.as_str()) != item)
            {
                self.set(slot, None);
            }
        }
        if config.enabled
            && now >= self.quiet_until
            && let Some(last) = &self.last
        {
            let (before, after) = (last.totals(), inventory.totals());
            let mut arrived = Vec::new();
            for slot in 0..SLOTS {
                let Some(stack) = inventory.slot(slot) else {
                    continue;
                };
                let grew = match last.slot(slot) {
                    None => true,
                    Some(was) => was.item == stack.item && stack.count > was.count,
                };
                let more = after.get(stack.item.as_str()).copied().unwrap_or(0)
                    > before.get(stack.item.as_str()).copied().unwrap_or(0);
                if grew && more {
                    arrived.push((slot, stack.item.clone()));
                }
            }
            for (slot, item) in arrived {
                let mark = Mark {
                    item,
                    since: now,
                    seen: false,
                };
                self.set(slot, Some(mark));
            }
        }
        if self.screen_open && !screen_open && config.clear.on_close() {
            for mark in &mut self.marks {
                if mark.as_ref().is_some_and(|m| m.seen) {
                    *mark = None;
                }
            }
        }
        if config.timeout > 0 {
            let life = u64::from(config.timeout) * TICKS_PER_SECOND;
            for mark in &mut self.marks {
                if mark
                    .as_ref()
                    .is_some_and(|m| now.saturating_sub(m.since) >= life)
                {
                    *mark = None;
                }
            }
        }
        self.screen_open = screen_open;
        self.last = Some(inventory);
    }
}

/// The game's menu slot id of tracked slot `slot` (main slots keep their
/// index; the off hand is 504).
pub fn slot_id(slot: usize) -> usize {
    if slot == OFFHAND {
        OFFHAND_SLOT_ID
    } else {
        slot
    }
}

// ---------------------------------------------------------------- drawing

/// A rectangle to fill, in GUI pixels: `[x, y, width, height]` and ARGB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub rect: [f32; 4],
    pub argb: u32,
}
/// A mark's fills: under the item (drawn before it) and over it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Paint {
    pub behind: Vec<Fill>,
    pub front: Vec<Fill>,
}

/// The pulse at `millis`: 0..=1, a triangle wave; always 1 without animation.
pub fn pulse(millis: u64, animate: bool) -> f32 {
    if !animate {
        return 1.0;
    }
    let half = PULSE_MILLIS / 2;
    let t = millis % PULSE_MILLIS;
    let up = if t < half { t } else { PULSE_MILLIS - t };
    up as f32 / half as f32
}
fn argb(alpha: f32, rgb: u32) -> u32 {
    let a = (alpha.clamp(0.0, 255.0)) as u32;
    (a << 24) | (rgb & 0x00FF_FFFF)
}
/// The four sides of the square ring `inset` pixels inside the cell at `at`.
fn ring(at: [f32; 2], inset: f32, argb: u32, out: &mut Vec<Fill>) {
    let [x, y] = at;
    let side = CELL - 2.0 * inset;
    let (x, y) = (x + inset, y + inset);
    let rects = [
        [x, y, side, 1.0],
        [x, y + side - 1.0, side, 1.0],
        [x, y + 1.0, 1.0, side - 2.0],
        [x + side - 1.0, y + 1.0, 1.0, side - 2.0],
    ];
    out.extend(rects.into_iter().map(|rect| Fill { rect, argb }));
}
/// A four-pointed star centred on pixel `c`, arms `arm` pixels long, with
/// a dark shadow one pixel down and right and a white heart.
fn star(c: [f32; 2], arm: f32, rgb: u32, out: &mut Vec<Fill>) {
    let [cx, cy] = c;
    let long = 2.0 * arm + 1.0;
    for (d, colour) in [(1.0, 0x9000_0000), (0.0, argb(255.0, rgb))] {
        out.push(Fill {
            rect: [cx - arm + d, cy + d, long, 1.0],
            argb: colour,
        });
        out.push(Fill {
            rect: [cx + d, cy - arm + d, 1.0, long],
            argb: colour,
        });
    }
    out.push(Fill {
        rect: [cx, cy, 1.0, 1.0],
        argb: 0xFFFF_FFFF,
    });
}

/// The fills of a mark of `style` in `rgb` on the slot at `at` (its top
/// left corner), at pulse `pulse`. `under_item` is false where the item is
/// already drawn (the HUD's hotbar): the glow's soft centre is then left out
/// so the item is not tinted.
pub fn paint(style: Style, rgb: u32, at: [f32; 2], pulse: f32, under_item: bool) -> Paint {
    let mut paint = Paint::default();
    let pulse = pulse.clamp(0.0, 1.0);
    let [x, y] = at;
    if matches!(style, Style::Glow | Style::GlowStar) {
        let strength = 0.55 + 0.45 * pulse;
        if under_item {
            paint.behind.push(Fill {
                rect: [x + 1.0, y + 1.0, CELL - 2.0, CELL - 2.0],
                argb: argb(96.0 * strength, rgb),
            });
        }
        let rings: &[f32] = if under_item {
            &[176.0, 104.0, 48.0]
        } else {
            &[176.0, 96.0]
        };
        for (inset, alpha) in rings.iter().enumerate() {
            ring(
                at,
                inset as f32,
                argb(alpha * strength, rgb),
                &mut paint.front,
            );
        }
    }
    if style == Style::Frame {
        let strength = 0.6 + 0.4 * pulse;
        ring(at, 0.0, argb(255.0 * strength, rgb), &mut paint.front);
    }
    if matches!(style, Style::Star | Style::GlowStar) {
        let arm = if pulse >= 0.5 { 2.0 } else { 1.0 };
        star([x + 3.0, y + 3.0], arm, rgb, &mut paint.front);
    }
    paint
}

/// `ItemStack.getRarity`'s name colour.
pub fn rarity_rgb(rarity: &str) -> u32 {
    match rarity {
        "uncommon" => 0xFFFF55,
        "rare" => 0x55FFFF,
        "epic" => 0xFF55FF,
        _ => 0xFFFFFF,
    }
}
/// The mark's RGB for `colour`; `rarity` is the item's (for [`Colour::Rarity`]).
pub fn colour_rgb(colour: Colour, rarity: &str) -> u32 {
    match colour {
        Colour::Gold => 0xFFC830,
        Colour::Rarity => rarity_rgb(rarity),
        Colour::White => 0xFFFFFF,
        Colour::Aqua => 0x40E0FF,
        Colour::Lime => 0x7CFF4F,
        Colour::Pink => 0xFF6FD0,
    }
}

/// `Hud.extractItemHotbar`'s item position of hotbar slot `slot` (0..9) or
/// the off hand ([`OFFHAND`]), on a HUD `width` x `height` GUI pixels;
/// the off hand sits left of the hotbar for a right-handed player.
pub fn hotbar_slot(width: f32, height: f32, slot: usize, right_handed: bool) -> Option<[f32; 2]> {
    let (w, h) = (width as i32, height as i32);
    let center = w / 2;
    let x = match slot {
        0..9 => center - 90 + slot as i32 * 20 + 2,
        OFFHAND if right_handed => center - 91 - 26,
        OFFHAND => center + 91 + 10,
        _ => return None,
    };
    Some([x as f32, (h - 16 - 3) as f32])
}

/// Whether the pointer at `cursor` is over the slot drawn at `at`.
pub fn inside(cursor: (f32, f32), at: [f32; 2]) -> bool {
    let [x, y] = at;
    cursor.0 >= x && cursor.0 < x + CELL && cursor.1 >= y && cursor.1 < y + CELL
}
/// Two GUI positions are the same slot (the game adds the same floats).
pub fn same_place(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
}

// ---------------------------------------------------------------- command

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Status,
    Clear,
    Reload,
}
/// `/itemspotlight ...`: None when the line is another command.
pub fn parse_command(line: &str) -> Option<Result<Command, &'static str>> {
    let mut words = line.trim().trim_start_matches('/').split_whitespace();
    if !words.next()?.eq_ignore_ascii_case(COMMAND) {
        return None;
    }
    let command = match (words.next(), words.next()) {
        (None | Some("status"), None) => Command::Status,
        (Some("clear"), None) => Command::Clear,
        (Some("reload"), None) => Command::Reload,
        _ => return Some(Err(USAGE)),
    };
    Some(Ok(command))
}
/// `/itemspotlight status`'s line.
pub fn status(loaded: &Loaded, marked: usize) -> String {
    match loaded {
        Err(error) => format!("{NAME} is off: {CONFIG_FILE}: {error}"),
        Ok(c) if !c.enabled => format!("{NAME} is off (settings)"),
        Ok(_) => format!("{NAME} is on: {marked} marked"),
    }
}

// ---------------------------------------------------------------- settings page

/// The configuration as read from [`CONFIG_FILE`]: Err when the file is broken
/// (the mod then marks nothing until it is fixed or reset).
pub type Loaded = Result<Config, String>;

/// A button of the settings page (Options > Mods > Item Spotlight > Settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// The file is broken: its error. A click reads it again.
    Broken,
    Enabled,
    Style,
    Colour,
    Hotbar,
    Inventory,
    Containers,
    Clear,
    Timeout,
    Animate,
    /// Take every mark away now.
    ClearNow,
    Defaults,
}
/// What a click on the settings page asks of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Write this configuration to the file; it applies at once.
    Save(Config),
    /// Read the file again (after editing it by hand).
    Reload,
    /// Take every mark away.
    ClearMarks,
}
/// The steps `timeout` cycles through, in seconds (0: never).
pub const TIMEOUT_STEPS: [u32; 7] = [0, 10, 30, 60, 120, 300, 600];
/// Longest error shown on the page (the whole one goes to stderr).
const ERROR_WIDTH: usize = 40;

fn step<T: Copy + PartialOrd>(steps: &[T], now: T) -> T {
    steps.iter().copied().find(|&s| s > now).unwrap_or(steps[0])
}
fn on_off(on: bool) -> &'static str {
    if on { "ON" } else { "OFF" }
}
fn seconds(s: u32) -> String {
    match s {
        0 => "never".into(),
        s if s % 60 == 0 => format!("{} min", s / 60),
        s => format!("{s} s"),
    }
}

/// The page's buttons, in order.
pub fn page(loaded: &Loaded) -> Vec<Setting> {
    if loaded.is_err() {
        return vec![Setting::Broken, Setting::Defaults];
    }
    vec![
        Setting::Enabled,
        Setting::Style,
        Setting::Colour,
        Setting::Hotbar,
        Setting::Inventory,
        Setting::Containers,
        Setting::Clear,
        Setting::Timeout,
        Setting::Animate,
        Setting::ClearNow,
        Setting::Defaults,
    ]
}

impl Setting {
    /// Its button's label; `marked` is how many slots are marked now.
    pub fn label(self, loaded: &Loaded, marked: usize) -> String {
        let c = match loaded {
            Ok(c) => c,
            Err(error) => {
                return match self {
                    Self::Defaults => "Reset to defaults".into(),
                    _ => {
                        let mut short: String = error.chars().take(ERROR_WIDTH).collect();
                        if short.len() < error.len() {
                            short = format!("{}...", short.trim_end());
                        }
                        format!("{CONFIG_FILE}: {short} (click to reload)")
                    }
                };
            }
        };
        match self {
            Self::Broken => String::new(),
            Self::Enabled => format!("Highlights: {}", on_off(c.enabled)),
            Self::Style => match c.style {
                Style::Glow => "Style: soft glow",
                Style::Frame => "Style: frame",
                Style::Star => "Style: twinkling star",
                Style::GlowStar => "Style: glow and star",
            }
            .into(),
            Self::Colour => match c.color {
                Colour::Gold => "Colour: gold",
                Colour::Rarity => "Colour: the item name's colour",
                Colour::White => "Colour: white",
                Colour::Aqua => "Colour: aqua",
                Colour::Lime => "Colour: lime",
                Colour::Pink => "Colour: pink",
            }
            .into(),
            Self::Hotbar => format!("Hotbar: {}", on_off(c.hotbar)),
            Self::Inventory => format!("Inventory screen: {}", on_off(c.inventory)),
            Self::Containers => format!("Chests and other screens: {}", on_off(c.containers)),
            Self::Clear => match c.clear {
                Clear::HoverOrClose => "Cleared by: pointing at it or closing the screen",
                Clear::Hover => "Cleared by: pointing at it",
                Clear::Close => "Cleared by: closing the screen it showed in",
                Clear::Never => "Cleared by: nothing (timeout or button)",
            }
            .into(),
            Self::Timeout => format!("Fades after: {}", seconds(c.timeout)),
            Self::Animate => format!("Animation: {}", on_off(c.animate)),
            Self::ClearNow => format!("Clear all marks now ({marked} marked)"),
            Self::Defaults => "Reset to defaults".into(),
        }
    }

    /// What a click on it does to `loaded`.
    pub fn click(self, loaded: &Loaded) -> Effect {
        let Ok(c) = loaded else {
            return match self {
                Self::Defaults => Effect::Save(Config::default()),
                _ => Effect::Reload,
            };
        };
        let mut c = c.clone();
        match self {
            Self::Broken => return Effect::Reload,
            Self::ClearNow => return Effect::ClearMarks,
            Self::Enabled => c.enabled = !c.enabled,
            Self::Style => {
                c.style = match c.style {
                    Style::Glow => Style::Frame,
                    Style::Frame => Style::Star,
                    Style::Star => Style::GlowStar,
                    Style::GlowStar => Style::Glow,
                }
            }
            Self::Colour => {
                c.color = match c.color {
                    Colour::Gold => Colour::Rarity,
                    Colour::Rarity => Colour::White,
                    Colour::White => Colour::Aqua,
                    Colour::Aqua => Colour::Lime,
                    Colour::Lime => Colour::Pink,
                    Colour::Pink => Colour::Gold,
                }
            }
            Self::Hotbar => c.hotbar = !c.hotbar,
            Self::Inventory => c.inventory = !c.inventory,
            Self::Containers => c.containers = !c.containers,
            Self::Clear => {
                c.clear = match c.clear {
                    Clear::HoverOrClose => Clear::Hover,
                    Clear::Hover => Clear::Close,
                    Clear::Close => Clear::Never,
                    Clear::Never => Clear::HoverOrClose,
                }
            }
            Self::Timeout => c.timeout = step(&TIMEOUT_STEPS, c.timeout),
            Self::Animate => c.animate = !c.animate,
            Self::Defaults => c = Config::default(),
        }
        Effect::Save(c)
    }
}

/// The settings page's labels (`native_settings`).
pub fn labels(loaded: &Loaded, marked: usize) -> Vec<String> {
    let page = page(loaded).into_iter();
    page.map(|s| s.label(loaded, marked)).collect()
}
/// A click on button `button` of the page `labels` gave (`native_settings_click`).
pub fn click(loaded: &Loaded, button: usize) -> Option<Effect> {
    page(loaded).get(button).map(|s| s.click(loaded))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inv(slots: &[(usize, &str, u32)]) -> Inventory {
        let mut inventory = Inventory {
            slots: vec![None; SLOTS],
            carried: None,
        };
        for &(i, item, count) in slots {
            inventory.slots[i] = Some(Stack::new(item, count));
        }
        inventory
    }
    fn carrying(mut inventory: Inventory, item: &str, count: u32) -> Inventory {
        inventory.carried = Some(Stack::new(item, count));
        inventory
    }
    /// A tracker past its grace period with `start` as the baseline, at
    /// tick `GRACE_TICKS + 1`.
    fn tracker(start: Inventory, config: &Config) -> (Tracker, u64) {
        let mut t = Tracker::new();
        t.observe(0, start.clone(), "overworld", false, config);
        t.observe(GRACE_TICKS + 1, start, "overworld", false, config);
        (t, GRACE_TICKS + 1)
    }
    fn marked(t: &Tracker) -> Vec<usize> {
        t.marks().into_iter().map(|(i, _)| i).collect()
    }

    #[test]
    fn a_picked_up_item_is_marked() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[(0, "minecraft:dirt", 10)]), &c);
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:dirt", 10), (5, "minecraft:diamond", 1)]),
            "overworld",
            false,
            &c,
        );
        assert_eq!(marked(&t), [5]);
        let mark = t.marked(5).unwrap();
        assert_eq!(mark.item, "minecraft:diamond");
        assert_eq!(mark.since, now + 1);
        assert!(!mark.seen);
    }

    #[test]
    fn a_stack_that_grows_is_marked_and_the_others_are_not() {
        let c = Config::default();
        let start = inv(&[(0, "minecraft:dirt", 10), (1, "minecraft:stone", 3)]);
        let (mut t, now) = tracker(start, &c);
        let after = inv(&[(0, "minecraft:dirt", 12), (1, "minecraft:stone", 3)]);
        t.observe(now + 1, after, "overworld", false, &c);
        assert_eq!(marked(&t), [0]);
    }

    #[test]
    fn using_up_items_marks_nothing() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[(0, "minecraft:bread", 5)]), &c);
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:bread", 4)]),
            "overworld",
            false,
            &c,
        );
        t.observe(now + 2, inv(&[]), "overworld", false, &c);
        assert!(marked(&t).is_empty());
    }

    #[test]
    fn moving_items_around_marks_nothing() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[(9, "minecraft:torch", 64)]), &c);
        // A Shift click from the inventory to the hotbar.
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:torch", 64)]),
            "overworld",
            true,
            &c,
        );
        assert!(marked(&t).is_empty());
        // Picked up on the cursor, then put down in another slot.
        let held = carrying(inv(&[]), "minecraft:torch", 64);
        t.observe(now + 2, held, "overworld", true, &c);
        t.observe(
            now + 3,
            inv(&[(20, "minecraft:torch", 64)]),
            "overworld",
            true,
            &c,
        );
        assert!(marked(&t).is_empty());
    }

    #[test]
    fn items_taken_with_the_cursor_are_not_marked_but_shift_clicked_ones_are() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[]), &c);
        // Out of a chest on the cursor, then put down: handled by the player.
        let held = carrying(inv(&[]), "minecraft:iron_ingot", 8);
        t.observe(now + 1, held, "overworld", true, &c);
        t.observe(
            now + 2,
            inv(&[(12, "minecraft:iron_ingot", 8)]),
            "overworld",
            true,
            &c,
        );
        assert!(marked(&t).is_empty());
        // Shift-clicked out of the chest straight into the inventory.
        t.observe(
            now + 3,
            inv(&[
                (12, "minecraft:iron_ingot", 8),
                (13, "minecraft:gold_ingot", 4),
            ]),
            "overworld",
            true,
            &c,
        );
        assert_eq!(marked(&t), [13]);
    }

    #[test]
    fn an_item_that_turns_into_another_in_place_is_not_marked() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[(0, "minecraft:bucket", 1)]), &c);
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:water_bucket", 1)]),
            "overworld",
            false,
            &c,
        );
        assert!(marked(&t).is_empty());
        // One of several buckets filled: the water bucket goes to a free slot.
        let (mut t, now) = tracker(inv(&[(0, "minecraft:bucket", 4)]), &c);
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:bucket", 3), (1, "minecraft:water_bucket", 1)]),
            "overworld",
            false,
            &c,
        );
        assert_eq!(marked(&t), [1]);
    }

    #[test]
    fn the_off_hand_is_tracked() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[(OFFHAND, "minecraft:torch", 10)]), &c);
        t.observe(
            now + 1,
            inv(&[(OFFHAND, "minecraft:torch", 11)]),
            "overworld",
            false,
            &c,
        );
        assert_eq!(marked(&t), [OFFHAND]);
        assert_eq!(slot_id(OFFHAND), 504);
        assert_eq!(slot_id(7), 7);
    }

    #[test]
    fn a_mark_goes_with_its_item() {
        let c = Config::default();
        let (mut t, now) = tracker(inv(&[]), &c);
        t.observe(
            now + 1,
            inv(&[(3, "minecraft:apple", 2)]),
            "overworld",
            false,
            &c,
        );
        assert_eq!(marked(&t), [3]);
        // Eating one keeps the mark; the last one takes it.
        t.observe(
            now + 2,
            inv(&[(3, "minecraft:apple", 1)]),
            "overworld",
            false,
            &c,
        );
        assert_eq!(marked(&t), [3]);
        t.observe(now + 3, inv(&[]), "overworld", false, &c);
        assert!(marked(&t).is_empty());
    }

    #[test]
    fn nothing_is_marked_in_the_grace_period_after_a_dimension_change() {
        let c = Config::default();
        let mut t = Tracker::new();
        t.observe(0, inv(&[]), "overworld", false, &c);
        // The server sends the inventory a moment after joining.
        t.observe(
            5,
            inv(&[(0, "minecraft:sword", 1), (1, "minecraft:dirt", 64)]),
            "overworld",
            false,
            &c,
        );
        assert!(marked(&t).is_empty());
        t.observe(
            GRACE_TICKS,
            inv(&[
                (0, "minecraft:sword", 1),
                (1, "minecraft:dirt", 64),
                (2, "minecraft:stick", 1),
            ]),
            "overworld",
            false,
            &c,
        );
        assert_eq!(marked(&t), [2]);
        // Into the nether: a new grace period.
        let now = GRACE_TICKS + 10;
        t.observe(
            now,
            inv(&[(2, "minecraft:stick", 1)]),
            "the_nether",
            false,
            &c,
        );
        t.observe(
            now + 1,
            inv(&[(2, "minecraft:stick", 1), (4, "minecraft:quartz", 3)]),
            "the_nether",
            false,
            &c,
        );
        assert_eq!(marked(&t), [2]);
        t.observe(
            now + GRACE_TICKS,
            inv(&[(2, "minecraft:stick", 1), (4, "minecraft:quartz", 4)]),
            "the_nether",
            false,
            &c,
        );
        assert_eq!(marked(&t), [2, 4]);
    }

    #[test]
    fn hovering_clears_a_mark_only_when_the_rule_says_so() {
        for (clear, cleared) in [
            (Clear::HoverOrClose, true),
            (Clear::Hover, true),
            (Clear::Close, false),
            (Clear::Never, false),
        ] {
            let c = Config {
                clear,
                ..Config::default()
            };
            let (mut t, now) = tracker(inv(&[]), &c);
            t.observe(
                now + 1,
                inv(&[(0, "minecraft:egg", 1)]),
                "overworld",
                false,
                &c,
            );
            assert_eq!(t.hover(0, &c), cleared, "{clear:?}");
            assert_eq!(t.marked(0).is_none(), cleared, "{clear:?}");
            assert!(!t.hover(1, &c));
        }
    }

    #[test]
    fn closing_a_screen_clears_the_marks_it_showed() {
        for (clear, cleared) in [
            (Clear::HoverOrClose, true),
            (Clear::Hover, false),
            (Clear::Close, true),
            (Clear::Never, false),
        ] {
            let c = Config {
                clear,
                ..Config::default()
            };
            let start = inv(&[]);
            let (mut t, now) = tracker(start, &c);
            let two = inv(&[(0, "minecraft:egg", 1), (30, "minecraft:feather", 1)]);
            t.observe(now + 1, two.clone(), "overworld", false, &c);
            t.observe(now + 2, two.clone(), "overworld", true, &c);
            // The screen only drew slot 30 (say, the hotbar row is off).
            t.saw(30);
            t.observe(now + 3, two, "overworld", false, &c);
            assert!(t.marked(0).is_some(), "{clear:?}");
            assert_eq!(t.marked(30).is_none(), cleared, "{clear:?}");
        }
    }

    #[test]
    fn marks_fade_after_the_timeout() {
        let c = Config {
            timeout: 10,
            ..Config::default()
        };
        let (mut t, now) = tracker(inv(&[]), &c);
        let one = inv(&[(0, "minecraft:egg", 1)]);
        t.observe(now + 1, one.clone(), "overworld", false, &c);
        t.observe(now + 200, one.clone(), "overworld", false, &c);
        assert_eq!(marked(&t), [0]);
        t.observe(now + 201, one, "overworld", false, &c);
        assert!(marked(&t).is_empty());
    }

    #[test]
    fn new_arrivals_refresh_a_mark() {
        let c = Config {
            timeout: 10,
            ..Config::default()
        };
        let (mut t, now) = tracker(inv(&[]), &c);
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:egg", 1)]),
            "overworld",
            false,
            &c,
        );
        t.saw(0);
        t.observe(
            now + 100,
            inv(&[(0, "minecraft:egg", 2)]),
            "overworld",
            false,
            &c,
        );
        let mark = t.marked(0).unwrap();
        assert_eq!((mark.since, mark.seen), (now + 100, false));
    }

    #[test]
    fn turning_it_off_clears_and_stops_marking() {
        let on = Config::default();
        let off = Config {
            enabled: false,
            ..Config::default()
        };
        let (mut t, now) = tracker(inv(&[]), &on);
        t.observe(
            now + 1,
            inv(&[(0, "minecraft:egg", 1)]),
            "overworld",
            false,
            &on,
        );
        assert_eq!(t.count(), 1);
        t.observe(
            now + 2,
            inv(&[(0, "minecraft:egg", 2)]),
            "overworld",
            false,
            &off,
        );
        assert_eq!(t.count(), 0);
        // Back on: what arrived while off is not new.
        t.observe(
            now + 3,
            inv(&[(0, "minecraft:egg", 2)]),
            "overworld",
            false,
            &on,
        );
        assert_eq!(t.count(), 0);
        t.clear_marks();
        t.reset();
        assert!(t.marks().is_empty());
    }

    #[test]
    fn short_inventories_are_padded() {
        let c = Config::default();
        let mut t = Tracker::new();
        let short = Inventory {
            slots: vec![None; 3],
            carried: None,
        };
        t.observe(0, short, "o", false, &c);
        t.observe(
            GRACE_TICKS,
            inv(&[(OFFHAND, "minecraft:egg", 1)]),
            "o",
            false,
            &c,
        );
        assert_eq!(marked(&t), [OFFHAND]);
    }

    #[test]
    fn config_defaults_and_round_trip() {
        assert_eq!(Config::from_json("{}").unwrap(), Config::default());
        let c = Config {
            style: Style::GlowStar,
            color: Colour::Rarity,
            clear: Clear::Never,
            timeout: 30,
            hotbar: false,
            ..Config::default()
        };
        assert_eq!(Config::from_json(&c.to_json()).unwrap(), c);
        let text = r#"{"style":"star","color":"aqua","clear":"hover","timeout":60}"#;
        let parsed = Config::from_json(text).unwrap();
        assert_eq!(parsed.style, Style::Star);
        assert_eq!(parsed.color, Colour::Aqua);
        assert_eq!(parsed.clear, Clear::Hover);
        assert_eq!(parsed.timeout, 60);
        assert!(c.to_json().contains("\"glowStar\""));
    }

    #[test]
    fn broken_config_files_are_errors() {
        assert!(Config::from_json(r#"{"colour":"gold"}"#).is_err());
        assert!(Config::from_json(r#"{"style":"sparkles"}"#).is_err());
        assert!(Config::from_json(r#"{"timeout":86401}"#).is_err());
        assert!(Config::from_json("{").is_err());
        assert!(Config::from_json(r#"{"timeout":86400}"#).is_ok());
    }

    #[test]
    fn screens_follow_their_switches() {
        let c = Config {
            containers: false,
            ..Config::default()
        };
        assert!(c.shows(ScreenKind::Hotbar));
        assert!(c.shows(ScreenKind::Inventory));
        assert!(!c.shows(ScreenKind::Container));
        let off = Config {
            enabled: false,
            ..Config::default()
        };
        assert!(!off.shows(ScreenKind::Hotbar));
    }

    fn within_cell(fills: &[Fill], at: [f32; 2]) -> bool {
        fills.iter().all(|f| {
            let [x, y, w, h] = f.rect;
            w > 0.0
                && h > 0.0
                && x >= at[0]
                && y >= at[1]
                && x + w <= at[0] + CELL + 1.0
                && y + h <= at[1] + CELL + 1.0
        })
    }

    #[test]
    fn every_style_stays_inside_its_slot() {
        let at = [100.0, 50.0];
        for style in [Style::Glow, Style::Frame, Style::Star, Style::GlowStar] {
            for pulse in [0.0, 0.3, 0.7, 1.0] {
                for under in [true, false] {
                    let p = paint(style, 0xFFC830, at, pulse, under);
                    assert!(!p.front.is_empty(), "{style:?}");
                    assert!(within_cell(&p.behind, at), "{style:?}");
                    assert!(within_cell(&p.front, at), "{style:?}");
                }
            }
        }
    }

    #[test]
    fn the_glow_tints_the_item_only_from_behind() {
        let at = [0.0, 0.0];
        let screen = paint(Style::Glow, 0xFFFFFF, at, 1.0, true);
        assert_eq!(screen.behind.len(), 1);
        assert_eq!(screen.front.len(), 12);
        let hud = paint(Style::Glow, 0xFFFFFF, at, 1.0, false);
        assert!(hud.behind.is_empty());
        assert_eq!(hud.front.len(), 8);
        // The rings only cover the slot's edge, never its middle.
        for f in &hud.front {
            let [x, y, w, h] = f.rect;
            assert!(w == 1.0 || h == 1.0);
            assert!(x <= 1.0 || y <= 1.0 || x + w >= 15.0 || y + h >= 15.0);
        }
        assert!(paint(Style::Frame, 0, at, 1.0, true).behind.is_empty());
    }

    #[test]
    fn colours_and_pulse() {
        let dim = paint(Style::Frame, 0x123456, [0.0, 0.0], 0.0, true);
        let bright = paint(Style::Frame, 0x123456, [0.0, 0.0], 1.0, true);
        assert_eq!(bright.front[0].argb, 0xFF12_3456);
        assert!(dim.front[0].argb >> 24 < bright.front[0].argb >> 24);
        assert_eq!(dim.front[0].argb & 0xFF_FFFF, 0x123456);
        assert_eq!(pulse(0, true), 0.0);
        assert_eq!(pulse(PULSE_MILLIS / 2, true), 1.0);
        assert_eq!(pulse(PULSE_MILLIS / 4, true), 0.5);
        assert_eq!(pulse(PULSE_MILLIS, true), 0.0);
        assert_eq!(pulse(123, false), 1.0);
        assert_eq!(colour_rgb(Colour::Rarity, "epic"), 0xFF55FF);
        assert_eq!(colour_rgb(Colour::Rarity, "common"), 0xFFFFFF);
        assert_eq!(colour_rgb(Colour::Rarity, "uncommon"), 0xFFFF55);
        assert_eq!(colour_rgb(Colour::Rarity, "rare"), 0x55FFFF);
        assert_eq!(colour_rgb(Colour::Gold, "epic"), 0xFFC830);
    }

    #[test]
    fn the_star_twinkles() {
        let big = paint(Style::Star, 0xFFC830, [0.0, 0.0], 1.0, true);
        let small = paint(Style::Star, 0xFFC830, [0.0, 0.0], 0.0, true);
        let width = |p: &Paint| p.front.iter().map(|f| f.rect[2]).fold(0.0, f32::max);
        assert_eq!(width(&big), 5.0);
        assert_eq!(width(&small), 3.0);
        // The heart is white, drawn last.
        assert_eq!(big.front.last().unwrap().argb, 0xFFFF_FFFF);
    }

    #[test]
    fn hotbar_positions_match_the_hud() {
        // A 427x240 GUI (1280x720 at scale 3): the hotbar is centred.
        assert_eq!(hotbar_slot(427.0, 240.0, 0, true), Some([125.0, 221.0]));
        assert_eq!(hotbar_slot(427.0, 240.0, 8, true), Some([285.0, 221.0]));
        assert_eq!(
            hotbar_slot(427.0, 240.0, OFFHAND, true),
            Some([96.0, 221.0])
        );
        assert_eq!(
            hotbar_slot(427.0, 240.0, OFFHAND, false),
            Some([314.0, 221.0])
        );
        assert_eq!(hotbar_slot(427.0, 240.0, 9, true), None);
    }

    #[test]
    fn hover_and_place_tests() {
        assert!(inside((10.0, 10.0), [10.0, 10.0]));
        assert!(inside((25.9, 25.9), [10.0, 10.0]));
        assert!(!inside((26.0, 10.0), [10.0, 10.0]));
        assert!(!inside((9.9, 10.0), [10.0, 10.0]));
        assert!(same_place([1.0, 2.0], [1.001, 2.0]));
        assert!(!same_place([1.0, 2.0], [1.0, 3.0]));
    }

    #[test]
    fn commands() {
        assert_eq!(parse_command("itemspotlight"), Some(Ok(Command::Status)));
        assert_eq!(
            parse_command("/itemspotlight status"),
            Some(Ok(Command::Status))
        );
        assert_eq!(
            parse_command("ItemSpotlight clear"),
            Some(Ok(Command::Clear))
        );
        assert_eq!(
            parse_command(" itemspotlight  reload "),
            Some(Ok(Command::Reload))
        );
        assert_eq!(parse_command("itemspotlight what"), Some(Err(USAGE)));
        assert_eq!(parse_command("itemspotlight clear now"), Some(Err(USAGE)));
        assert_eq!(parse_command("itemspotlights"), None);
        assert_eq!(parse_command("give @s dirt"), None);
        assert_eq!(parse_command(""), None);
        let on = Ok(Config::default());
        assert_eq!(status(&on, 3), "Item Spotlight is on: 3 marked");
        let off = Ok(Config {
            enabled: false,
            ..Config::default()
        });
        assert!(status(&off, 0).contains("off"));
        assert!(status(&Err("bad".into()), 0).contains("config.json: bad"));
    }

    #[test]
    fn every_button_has_a_label_and_a_click() {
        let loaded = Ok(Config::default());
        let labels = labels(&loaded, 2);
        assert_eq!(labels.len(), page(&loaded).len());
        assert!(labels.iter().all(|l| !l.is_empty()));
        assert_eq!(labels[0], "Highlights: ON");
        assert_eq!(labels[9], "Clear all marks now (2 marked)");
        for button in 0..labels.len() {
            assert!(click(&loaded, button).is_some());
        }
        assert_eq!(click(&loaded, labels.len()), None);
        assert_eq!(click(&loaded, 9), Some(Effect::ClearMarks));
        assert_eq!(click(&loaded, 10), Some(Effect::Save(Config::default())));
    }

    #[test]
    fn clicks_cycle_every_value_back_to_the_start() {
        let start = Config::default();
        let cycles = [
            (Setting::Enabled, 2),
            (Setting::Style, 4),
            (Setting::Colour, 6),
            (Setting::Hotbar, 2),
            (Setting::Inventory, 2),
            (Setting::Containers, 2),
            (Setting::Clear, 4),
            (Setting::Timeout, TIMEOUT_STEPS.len()),
            (Setting::Animate, 2),
        ];
        for (setting, n) in cycles {
            let mut c = start.clone();
            let mut labels = Vec::new();
            for _ in 0..n {
                labels.push(setting.label(&Ok(c.clone()), 0));
                match setting.click(&Ok(c.clone())) {
                    Effect::Save(next) => c = next,
                    other => panic!("{setting:?}: {other:?}"),
                }
            }
            assert_eq!(c, start, "{setting:?}");
            labels.sort();
            labels.dedup();
            assert_eq!(labels.len(), n, "{setting:?} labels must differ");
        }
    }

    #[test]
    fn timeout_steps_and_labels() {
        let label = |timeout| {
            Setting::Timeout.label(
                &Ok(Config {
                    timeout,
                    ..Config::default()
                }),
                0,
            )
        };
        assert_eq!(label(0), "Fades after: never");
        assert_eq!(label(30), "Fades after: 30 s");
        assert_eq!(label(120), "Fades after: 2 min");
        // A hand-written value between steps goes to the next one up.
        assert_eq!(step(&TIMEOUT_STEPS, 45), 60);
        assert_eq!(step(&TIMEOUT_STEPS, 600), 0);
    }

    #[test]
    fn a_broken_file_offers_a_reload_and_a_reset() {
        let broken: Loaded = Err("unknown field `colour`, expected one of ...".repeat(2));
        let labels = labels(&broken, 0);
        assert_eq!(labels.len(), 2);
        assert!(labels[0].starts_with("config.json: unknown field"));
        assert!(labels[0].ends_with("... (click to reload)"));
        assert_eq!(labels[1], "Reset to defaults");
        assert_eq!(click(&broken, 0), Some(Effect::Reload));
        assert_eq!(click(&broken, 1), Some(Effect::Save(Config::default())));
    }
}
