// native_hooks::Game::native_hud (a hook point): the marks on the hotbar and
// the off hand slot, drawn after the hotbar's items.
fn itemspotlight_hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    game.itemspotlight_hud(batch);
}
