// SPDX-License-Identifier: MIT
// Dragging on SuperCapes's settings card: its sliders and scrollbar.
fn supercapes_card_options_drag(game: &mut crate::game::Game<'_>) -> ::core::ops::ControlFlow<Result<()>> {
    if !crate::game::supercapes_card::active(game) {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::supercapes_card::drag(game))
}
