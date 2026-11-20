// SPDX-License-Identifier: MIT
fn supercapes_pause_row(
    buttons: Vec<(f32, f32, f32, &'static str, u8, bool)>,
) -> Vec<(f32, f32, f32, &'static str, u8, bool)> {
    // A real pause-menu button (the game draws it with its own button
    // sprites), full width, right above Save and Quit to Title.
    let mut buttons = buttons;
    mod_supercapes::logic::layout::insert_pause_row(&mut buttons);
    buttons
}
