// native_hooks::Game::native_settings_click (a hook point): a click on Item
// Item New's settings page changes the setting, keeps it and applies it.
fn itemnew_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == crate::game::itemnew::ID {
        game.itemnew_settings_click(button);
    }
}
