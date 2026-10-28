// native_hooks::Game::native_command (a hook point): run `/itemnew`,
// leave everything else to the game (and to other mods).
fn itemnew_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    if !*ran && game.itemnew_command(command) {
        *ran = true;
    }
}
