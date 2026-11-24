// SPDX-License-Identifier: MIT
// The mouse buttons on SuperCapes's settings card.
fn supercapes_card_options_mouse(
    game: &mut crate::game::Game<'_>,
    pressed: bool,
    right: bool,
) -> ::core::ops::ControlFlow<Result<bool>> {
    if !crate::game::supercapes_card::active(game) {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::supercapes_card::mouse(game, pressed, right).map(|()| true))
}
