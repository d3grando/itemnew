// native_hooks::Game::native_settings_click (a hook point): a click on Item
// Spotlight's settings page changes the setting, keeps it and applies it.
fn itemspotlight_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::itemspotlight::ID {
        game.itemspotlight_settings_click(button);
    }
}
