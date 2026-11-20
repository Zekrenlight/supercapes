// SPDX-License-Identifier: MIT
fn supercapes_cape_layer(
    game: &Game<'_>,
    m: &Mob,
    chest: Option<&str>,
    camera: glam::DVec3,
    partial: f32,
    light: u32,
    out: &mut Vec<sunrise_client_render::render_geometry::Vertex>,
) -> ::core::ops::ControlFlow<()> {
    // One cape per player: when the cloth draws this player's cape, the
    // vanilla cape layer stays out.
    if crate::game::native_hooks::render_frame_pre::supercapes_native::cloth_replaces_vanilla(game, m) {
        return ::core::ops::ControlFlow::Break(());
    }
    // Cloth off: the vanilla layer draws the winning picture, even if the
    // game's skin refresh put the player's own cape back this frame.
    if let Some(sprite) =
        crate::game::native_hooks::render_frame_pre::supercapes_native::cape_override(game, m, false)
    {
        let mut ours = m.clone();
        ours.cape = Some(sprite);
        game.player_cape_geometry(&ours, chest, camera, partial, light, out);
        return ::core::ops::ControlFlow::Break(());
    }
    ::core::ops::ControlFlow::Continue(())
}
