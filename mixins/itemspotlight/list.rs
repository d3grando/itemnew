// native_hooks::Game::native_mods (a hook point): Item Spotlight's row in Options > Mods.
fn itemspotlight_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::itemspotlight as is;
    mods.push(crate::game::native_hooks::NativeMod {
        id: is::ID.into(),
        title: format!("{} {}", is::NAME, is::VERSION),
        description: is::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
