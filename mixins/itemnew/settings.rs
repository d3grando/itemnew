// native_hooks::Game::native_settings (a hook point): Item New's settings page.
fn itemnew_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::itemnew::ID {
        buttons.extend(game.itemnew_settings());
    }
}
