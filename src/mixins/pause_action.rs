// SPDX-License-Identifier: MIT
fn supercapes_pause_action(hit: u8) -> ::core::ops::ControlFlow<Result<()>> {
    if hit == mod_supercapes::logic::layout::PAUSE_ACTION {
        mod_supercapes::session::open_overlay();
        ::core::ops::ControlFlow::Break(Ok(()))
    } else {
        ::core::ops::ControlFlow::Continue(())
    }
}
