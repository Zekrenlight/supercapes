// SPDX-License-Identifier: MIT
fn supercapes_frame(game: &mut Game<'_>) {
    crate::game::native_hooks::render_frame_pre::supercapes_native::frame(game);
}
