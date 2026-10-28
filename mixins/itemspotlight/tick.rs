// native_hooks::Game::native_tick (a hook point): the window compares its
// inventory with the last tick's and marks what arrived.
fn itemspotlight_tick(game: &mut crate::game::Game<'_>) {
    game.itemspotlight_tick();
}
