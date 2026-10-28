// native_hooks::Game::native_settings (a hook point): Item Spotlight's settings page.
fn itemspotlight_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id == crate::game::itemspotlight::ID {
        buttons.extend(game.itemspotlight_settings());
    }
}
