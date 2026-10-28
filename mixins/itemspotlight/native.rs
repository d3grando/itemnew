// Item Spotlight's adapter to the game (Horizon 142388e1c136e622652ec7505af15d8ad195e041,
// see horizon-mixin.toml). Injected as `crate::game::itemspotlight_native`; feeds
// the window's inventory to the game-independent tracker in
// `crate::game::itemspotlight` (runtime.rs) and paints its marks, and keeps the
// settings where the native hook points' helpers do.
use super::itemspotlight::{
    self as is, Colour, Command, Config, Effect, Fill, Inventory, Loaded, ScreenKind, Stack,
    Tracker,
};
use super::{Game, Screen};
use horizon_client::{hud::HudBatch, items::ItemStack};

/// The window's tracker and configuration. Only the window's game uses
/// them (single player's server thread has no screens or HUD), but the
/// draw hook only has `&Game`, so they live here rather than in the game.
struct Shared {
    tracker: Tracker,
    /// `config.json` as last read; None until something needs it.
    config: Option<Loaded>,
}
static SHARED: std::sync::Mutex<Shared> = std::sync::Mutex::new(Shared {
    tracker: Tracker::new(),
    config: None,
});
thread_local! {
    // Set while `draw_item_model` draws a marked item for us (its HEAD hook
    // then lets it through).
    static DRAWING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
/// Never held while the game runs a hook of ours.
fn shared<T>(run: impl FnOnce(&mut Shared) -> T) -> T {
    let mut shared = SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    run(&mut shared)
}

/// `config.json` in the mod's settings folder; missing is the defaults.
fn read_config(game: &Game<'_>) -> Loaded {
    match game.native_setting(is::ID, is::CONFIG_FILE) {
        Some(text) => Config::from_json(&text).map_err(|e| format!("{}: {e}", is::CONFIG_FILE)),
        None => Ok(Config::default()),
    }
}
/// Puts `loaded` in effect.
fn apply(loaded: Loaded) {
    if let Err(error) = &loaded {
        eprintln!("Item Spotlight disabled: {error}");
    }
    shared(|s| s.config = Some(loaded));
}
/// The configuration as read (once).
fn loaded(game: &Game<'_>) -> Loaded {
    if let Some(loaded) = shared(|s| s.config.clone()) {
        return loaded;
    }
    let read = read_config(game);
    shared(|s| {
        s.config
            .get_or_insert_with(|| {
                if let Err(error) = &read {
                    eprintln!("Item Spotlight disabled: {error}");
                }
                read
            })
            .clone()
    })
}
/// The configuration in effect: a broken file marks nothing (fails closed).
fn config(game: &Game<'_>) -> Config {
    loaded(game).unwrap_or_else(|_| Config {
        enabled: false,
        ..Config::default()
    })
}
/// The window's game, with the mod left on in Options > Mods: every hook
/// but the list and settings ones passes through untouched otherwise.
fn on(game: &Game<'_>) -> bool {
    game.native_window() && game.native_on(is::ID)
}

fn stack(stack: Option<&ItemStack>) -> Option<Stack> {
    stack.map(|s| Stack::new(&s.item, s.count))
}
/// The player's 36 main slots, the off hand and the cursor.
fn inventory(game: &Game<'_>) -> Inventory {
    let main = (0..is::MAIN_SLOTS).map(|i| stack(game.inventory.get(i).and_then(Option::as_ref)));
    Inventory {
        slots: main.chain([stack(game.offhand.as_ref())]).collect(),
        carried: stack(game.carried.as_ref()),
    }
}
/// The tracked slot's stack in the game.
fn tracked<'g>(game: &'g Game<'_>, slot: usize) -> Option<&'g ItemStack> {
    if slot == is::OFFHAND {
        game.offhand.as_ref()
    } else {
        game.inventory.get(slot).and_then(Option::as_ref)
    }
}
/// The open screen that shows the player's slots, if any: the player's own
/// inventory (and the creative screen), or any other (a container, a
/// crafting table, a workstation, a villager, a mount).
fn screen_kind(game: &Game<'_>) -> Option<ScreenKind> {
    match game.screen {
        Screen::Creative => Some(ScreenKind::Inventory),
        Screen::Inventory
            if game.container.is_none()
                && game.station.is_none()
                && game.craft_size == 2
                && !game.mount_active() =>
        {
            Some(ScreenKind::Inventory)
        }
        Screen::Inventory => Some(ScreenKind::Container),
        _ => None,
    }
}
/// The mark's colour for `stack`.
fn rgb(game: &Game<'_>, config: &Config, stack: &ItemStack) -> u32 {
    if config.color != Colour::Rarity {
        return is::colour_rgb(config.color, "");
    }
    let default = game.item_rarity(&stack.item);
    let rarity = horizon_client::item_tooltip::rarity(stack, &default);
    is::colour_rgb(config.color, rarity)
}
fn pulse(game: &Game<'_>, config: &Config) -> f32 {
    let millis = game.started.elapsed().as_millis();
    is::pulse(u64::try_from(millis).unwrap_or(0), config.animate)
}
fn fill(game: &Game<'_>, batch: &mut HudBatch, fills: &[Fill]) {
    for f in fills {
        game.native_fill(batch, f.rect, f.argb);
    }
}

impl Game<'_> {
    /// `native_tick` (a hook point): compare the window's inventory with the
    /// last tick's and mark what arrived.
    pub(super) fn itemspotlight_tick(&mut self) {
        if !self.native_window() {
            return;
        }
        if !self.native_on(is::ID) || self.player.is_none() {
            shared(|s| s.tracker.reset());
            return;
        }
        let config = config(self);
        let inventory = inventory(self);
        let open = matches!(self.screen, Screen::Inventory | Screen::Creative);
        let (now, context) = (self.ticks, self.dim.id());
        shared(|s| s.tracker.observe(now, inventory, context, open, &config));
    }

    /// `native_hud` (a hook point), after the hotbar's items: the marks on
    /// the hotbar and the off hand slot.
    pub(super) fn itemspotlight_hud(&self, batch: &mut HudBatch) {
        if !on(self) || self.player.is_none() || self.spectator() {
            return;
        }
        let marks = shared(|s| s.tracker.marks());
        let hotbar = marks.iter().filter(|(i, _)| *i < 9 || *i == is::OFFHAND);
        let hotbar: Vec<_> = hotbar.collect();
        if hotbar.is_empty() {
            return;
        }
        let config = config(self);
        if !config.shows(ScreenKind::Hotbar) {
            return;
        }
        let pulse = pulse(self, &config);
        let right_handed = !self.settings.main_hand_left;
        for (slot, item) in hotbar {
            let Some(stack) = tracked(self, *slot).filter(|s| *s.item == **item) else {
                continue;
            };
            let Some(at) = is::hotbar_slot(batch.width, batch.height, *slot, right_handed) else {
                continue;
            };
            let paint = is::paint(config.style, rgb(self, &config, stack), at, pulse, false);
            fill(self, batch, &paint.behind);
            fill(self, batch, &paint.front);
        }
    }

    /// `item_draw::Game::draw_item_model`'s HEAD: an item drawn in a screen
    /// at the place of a marked player slot gets its mark (under and over
    /// the item); true when it was drawn here. Pointing at it clears it.
    pub(super) fn itemspotlight_draw_item(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
    ) -> bool {
        if DRAWING.get() || !on(self) {
            return false;
        }
        let Some(kind) = screen_kind(self) else {
            return false;
        };
        let marks = shared(|s| s.tracker.marks());
        if !marks.iter().any(|(_, item)| *item == *stack.item) {
            return false;
        }
        let config = config(self);
        if !config.shows(kind) {
            return false;
        }
        let at = [x, y];
        // The stack on the cursor is drawn at the pointer, not in a slot.
        let pointer = [self.cursor.0 - 8.0, self.cursor.1 - 8.0];
        if self.carried.is_some() && is::same_place(at, pointer) {
            return false;
        }
        let (ox, oy) = self.inventory_origin(batch);
        let shown = self.slots();
        let slot = marks.iter().find_map(|(slot, item)| {
            let id = is::slot_id(*slot);
            let here = *item == *stack.item && shown.contains(&id) && {
                let (sx, sy) = self.slot_position(id);
                is::same_place(at, [ox + sx, oy + sy])
            };
            here.then_some(*slot)
        });
        let Some(slot) = slot else {
            return false;
        };
        let hovered = is::inside(self.cursor, at);
        let cleared = shared(|s| {
            if hovered && s.tracker.hover(slot, &config) {
                return true;
            }
            s.tracker.saw(slot);
            false
        });
        if cleared {
            return false;
        }
        let paint = is::paint(
            config.style,
            rgb(self, &config, stack),
            at,
            pulse(self, &config),
            true,
        );
        fill(self, batch, &paint.behind);
        struct Drawing;
        impl Drop for Drawing {
            fn drop(&mut self) {
                DRAWING.set(false);
            }
        }
        DRAWING.set(true);
        {
            let _drawing = Drawing;
            self.draw_item_model(batch, stack, x, y);
        }
        fill(self, batch, &paint.front);
        true
    }

    /// `native_command` (a hook point): `/itemspotlight [status|clear|reload]`,
    /// run by the window itself (on any server); true when it was ours.
    pub(super) fn itemspotlight_command(&mut self, command: &str) -> bool {
        if !on(self) {
            return false;
        }
        let Some(parsed) = is::parse_command(command) else {
            return false;
        };
        let message = match parsed {
            Err(usage) => usage.to_owned(),
            Ok(Command::Status) => {
                let marked = shared(|s| s.tracker.count());
                is::status(&loaded(self), marked)
            }
            Ok(Command::Clear) => {
                let marked = shared(|s| {
                    let marked = s.tracker.count();
                    s.tracker.clear_marks();
                    marked
                });
                format!("{}: cleared {marked} marks", is::NAME)
            }
            Ok(Command::Reload) => {
                let read = read_config(self);
                let message = match &read {
                    Ok(_) => format!("{}: settings reloaded", is::NAME),
                    Err(error) => format!("{}: {error}", is::NAME),
                };
                apply(read);
                message
            }
        };
        self.native_chat(&message);
        true
    }

    /// `native_settings` (a hook point): the settings page's buttons.
    pub(super) fn itemspotlight_settings(&self) -> Vec<String> {
        let marked = shared(|s| s.tracker.count());
        is::labels(&loaded(self), marked)
    }

    /// `native_settings_click` (a hook point): change the setting, write
    /// `config.json` and put it in effect at once (or read the file again,
    /// or take the marks away).
    pub(super) fn itemspotlight_settings_click(&mut self, button: usize) {
        match is::click(&loaded(self), button) {
            Some(Effect::Save(config)) => {
                self.native_set_setting(is::ID, is::CONFIG_FILE, &config.to_json());
                apply(Ok(config));
            }
            Some(Effect::Reload) => apply(read_config(self)),
            Some(Effect::ClearMarks) => shared(|s| s.tracker.clear_marks()),
            None => {}
        }
    }
}
