// SPDX-License-Identifier: MIT
// Keys on SuperCapes's settings card; Escape (search let go) is Sunrise's, to leave.
fn supercapes_card_options_key(
    game: &mut crate::game::Game<'_>,
    key: KeyCode,
    text: Option<&str>,
) -> ::core::ops::ControlFlow<Result<bool>> {
    if !crate::game::supercapes_card::active(game) {
        return ::core::ops::ControlFlow::Continue(());
    }
    match crate::game::supercapes_card::key(game, key, text) {
        Some(done) => ::core::ops::ControlFlow::Break(done.map(|()| true)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
