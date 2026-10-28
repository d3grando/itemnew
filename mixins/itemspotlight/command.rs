// native_hooks::Game::native_command (a hook point): run `/itemspotlight`,
// leave everything else to the game (and to other mods).
fn itemspotlight_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && game.itemspotlight_command(command) {
        *ran = true;
    }
}
