// SPDX-License-Identifier: MIT
fn supercapes_elytra(
    game: &Game<'_>,
    m: &Mob,
    chest: Option<&str>,
    camera: glam::DVec3,
    partial: f32,
    light: u32,
    out: &mut Vec<sunrise_client_render::render_geometry::Vertex>,
) -> ::core::ops::ControlFlow<()> {
    // "Elytra uses cape" on: the wings take the winning cape picture (its
    // elytra area, 22..46 x 0..22 of the 64x32 layout an OptiFine cape keeps
    // after fitting), not the player's own cape the game may have put back.
    // Off: no cape at all, so the wings show the plain elytra texture.
    if let Some(cape) =
        crate::game::native_hooks::render_frame_pre::supercapes_native::elytra_cape(game, m)
    {
        let mut ours = m.clone();
        ours.cape = cape;
        game.player_elytra_geometry(&ours, chest, camera, partial, light, out);
        return ::core::ops::ControlFlow::Break(());
    }
    ::core::ops::ControlFlow::Continue(())
}
