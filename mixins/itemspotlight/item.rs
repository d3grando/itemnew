// item_draw::Game::draw_item_model HEAD: a marked player slot in a screen
// gets its mark under and over its item (drawn here, so the game's own draw
// is skipped); every other item is left to the game.
fn itemspotlight_item(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    stack: &horizon_client::items::ItemStack,
    x: f32,
    y: f32,
) -> core::ops::ControlFlow<()> {
    if game.itemspotlight_draw_item(batch, stack, x, y) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
