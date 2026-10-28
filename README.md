# Item Spotlight

A **native** Horizon mod (horizon-mixins source mixin): items that just arrived
in your inventory get a soft glow (or a frame, or a twinkling star) on their
slot, in the hotbar and in every inventory screen, until you have seen them:
point at the slot, or close the screen it showed in. It is all configurable, in
game: Options > Mods > Item Spotlight > Settings.

Inspired by Item Highlighter (https://modrinth.com/mod/item-highlighter); not
affiliated, no code from it.

> **Native mods are not sandboxed.** Unlike `.hmod` (Wasm) mods, a native mod is
> Rust source compiled into your own Horizon executable and runs with your user
> rights. Read the source (it is small: `mixins/itemspotlight/`) before building.

## Behaviour

Once a tick the mod compares your inventory (the 36 main slots and the off
hand) with the last tick's. A slot is **marked** when it gained items (it was
empty, or holds more of the same item) and you now have more of that item in
total, counting the stack on the cursor. So these are marked:

- items you pick up from the ground, get from `/give`, a villager, a fox, a
  fishing rod...;
- items Shift-clicked into your inventory from a chest, a furnace, a crafting
  result, a villager trade;
- a stack that grows (64 dirt becoming 66 marks that slot).

And these are not:

- items moved around your inventory, Shift-clicked between hotbar and
  inventory, or taken on the cursor and put down by hand (you just handled
  them);
- an item that turns into another in its slot (a bucket filled with water, a
  potion drunk);
- what is there when you join a world or change dimension: for 3 seconds after
  that nothing is marked, so the inventory a server sends at login does not
  light up.

A mark goes away when its item leaves the slot (used up, dropped, moved), and
by the **clear rule** (default: either of the first two):

- **pointing at it** with the mouse in a screen;
- **closing a screen** it showed in (marks that screen never showed stay);
- after a **timeout**, if one is set;
- the settings page's *Clear all marks now* or `/itemspotlight clear`.

Where marks show (each can be turned off):

- the **hotbar** and the off-hand slot of the HUD (not under F1, not as a
  spectator);
- the **inventory screen**: your own inventory and the creative screen (its
  inventory tab, and the hotbar row of its item tabs);
- **chests and other screens**: the player slots of chests, barrels, shulker
  boxes, ender chests, hoppers, dispensers, furnaces, brewing stands, crafting
  tables, anvils, grindstones and the other workstations, beacons, villagers and
  mounts.

In a screen the glow sits under the item and its bright edge over it; on the
HUD's hotbar, where the item is already drawn, only the edge shows, so the item
is never tinted. Marks pulse gently (the star twinkles) unless the animation is
off. The colour is gold by default, or the colour of the item's name (white,
yellow, aqua or pink, by rarity, as its tooltip shows it), or one of four
fixed colours.

The marks are kept in memory for the game session only.

## Commands

| Command | Effect |
|---|---|
| `/itemspotlight` or `/itemspotlight status` | Whether it is on, and how many slots are marked |
| `/itemspotlight clear` | Take every mark away |
| `/itemspotlight reload` | Re-read `config.json` |

The command runs in your own game (it is never sent to the server), so it works
on any server.

## Settings

**Options > Mods** lists Item Spotlight with its **Settings** button. The row's
own button turns the mod off (it then does nothing at all: no tracking, no
marks, no command; the launcher's next build leaves it out) and on again. The
settings page has one button per setting; a click moves it to its next value,
and it applies at once, no restart:

| Button | Key | Values a click cycles through |
|---|---|---|
| Highlights: ON | `enabled` | ON, OFF |
| Style: soft glow | `style` | soft glow, frame, twinkling star, glow and star |
| Colour: gold | `color` | gold, the item name's colour, white, aqua, lime, pink |
| Hotbar: ON | `hotbar` | ON, OFF |
| Inventory screen: ON | `inventory` | ON, OFF |
| Chests and other screens: ON | `containers` | ON, OFF |
| Cleared by: pointing at it or closing the screen | `clear` | pointing at it or closing the screen, pointing at it, closing the screen it showed in, nothing |
| Fades after: never | `timeout` | never, 10 s, 30 s, 1 min, 2 min, 5 min, 10 min |
| Animation: ON | `animate` | ON, OFF |
| Clear all marks now (N marked) | | takes every mark away |
| Reset to defaults | | every setting above back to its default |

If the file is broken, the page shows its error (click to read the file again)
and a reset.

## Configuration file

The settings page writes, and the mod reads, one JSON file in the mod's
settings folder, beside the game's `options.txt`:

```
horizon-mods/.horizon-mod-data/itemspotlight/config.json
```

Every key is optional; a missing file is the defaults. Unknown keys or values,
or a `timeout` above 86400, turn the marks off (the page shows why, and the game
prints `Item Spotlight disabled: ...` to stderr) until the file is fixed or
reset. Edit it by hand for a timeout between the page's steps, then run
`/itemspotlight reload`.

```json
{
  "enabled": true,
  "style": "glow",
  "color": "gold",
  "hotbar": true,
  "inventory": true,
  "containers": true,
  "clear": "hoverOrClose",
  "timeout": 0,
  "animate": true
}
```

| Key | Default | Values |
|---|---|---|
| `enabled` | `true` | Global switch |
| `style` | `"glow"` | `"glow"`, `"frame"`, `"star"`, `"glowStar"` |
| `color` | `"gold"` | `"gold"`, `"rarity"` (the item name's colour), `"white"`, `"aqua"`, `"lime"`, `"pink"` |
| `hotbar` | `true` | Marks on the HUD's hotbar and off-hand slot |
| `inventory` | `true` | Marks in your inventory screen and the creative screen |
| `containers` | `true` | Marks in the player slots of every other screen |
| `clear` | `"hoverOrClose"` | `"hoverOrClose"`, `"hover"`, `"close"`, `"never"` |
| `timeout` | `0` | Seconds a mark lasts at most, 0..=86400; `0` is no limit |
| `animate` | `true` | Pulse and twinkle |

## Compatibility

Horizon's native mods can use **hook points** (`game/native_hooks.rs`,
`NATIVE_HOOKS_API`): functions kept stable from release to release. Item
Spotlight uses them for everything they cover:

- `native_tick`: the window's inventory, once a tick;
- `native_hud`: the marks on the HUD's hotbar, drawn with `native_fill`;
- `native_command`: `/itemspotlight`, answered with `native_chat`;
- `native_mods`, `native_settings`, `native_settings_click`: the row in Options >
  Mods and the settings page; `native_setting`/`native_set_setting` for the file,
  `native_on`/`native_window` for the off switch and the window's own game.

There is no hook point for the items a screen draws in its slots, so Item
Spotlight hooks one game function directly and is **pinned** to one Horizon
commit:

| Hook | Kind | Why it is not a hook point |
|---|---|---|
| `item_draw::Game::draw_item_model` | cancellable HEAD | The marks in screens. `native_hud` draws only the HUD, before any screen, so it cannot reach slots, and nothing tells a mod where a screen's slots are drawn. Every screen draws a slot's item through this one function (`draw_slot_item` and `draw_item` both end in it), with the item and its place, so the hook recognises a marked player slot by its place (`inventory_origin` + `slot_position`, the game's own layout) and its item. It draws the glow, then the item itself, then the edge or the star, and skips the game's draw (Break); any other item is left to the game (Continue). `draw_slot_item` itself cannot be hooked: its body has a macro, which horizon-mixins refuses. |

Pinned: Horizon `142388e1c136e622652ec7505af15d8ad195e041`, horizon-mixins
`0d6b103c70633d8fd9443d8d0acd3aadaacedb06`, game 26.3, the same as TreeFall
0.3.0 and Mason 0.2.0, so all three build into one game. `horizon-native.json`
therefore names no `hooks` version. Planned together with TreeFall and Mason
(`horizon-mixins plan`: 23 injections, 14 modules, no conflict): the three share
only the non-cancellable hook points; `draw_item_model` is Item Spotlight's
alone.

Single player: the mod works in the window's own game only (where the HUD and
screens are); single player's server thread passes through.

## Build and try

Item Spotlight is a source bundle; the launcher compiles it into a custom game:

```sh
horizon-launcher native-build <this folder> --version v0.2.97 --trust-native-code
```

`--version` is the installed release whose game files the build reuses; the
code is the pinned commit's whatever the release. `<this folder>` may also be a
folder holding several native mods.

## Development

`mixins/itemspotlight/`:

- `horizon-mixin.toml`: two support modules and seven HEAD hooks (above).
- `runtime.rs`: all decisions (what counts as arrived, the clear rules, the
  timeout, the marks' shapes and colours, the hotbar layout, the command, the
  config and the settings page's buttons) as pure code, with unit tests. It is
  both this repository's Cargo library and the injected
  `crate::game::itemspotlight` module.
- `native.rs`: the adapter to the game (`crate::game::itemspotlight_native`).
- `item.rs`, `tick.rs`, `hud.rs`, `command.rs`, `list.rs`, `settings.rs`,
  `click.rs`: the hook helpers.

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
python tools/pack_hnmod.py   # dist/itemspotlight-<version>.hnmod
```

`native.rs` and the hook helpers compile only inside Horizon; check their
formatting with `rustfmt --edition 2024 --check mixins/itemspotlight/*.rs`.

MIT licensed.
