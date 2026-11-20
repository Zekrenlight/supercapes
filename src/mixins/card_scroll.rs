// SPDX-License-Identifier: MIT
// The wheel on SuperCapes's settings card.
fn supercapes_card_options_scroll(
    game: &mut crate::game::Game<'_>,
    amount: f32,
) -> ::core::ops::ControlFlow<Result<()>> {
    if !crate::game::supercapes_card::active(game) {
        return ::core::ops::ControlFlow::Continue(());
    }
    ::core::ops::ControlFlow::Break(crate::game::supercapes_card::scroll(game, amount))
}
