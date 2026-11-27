// SPDX-License-Identifier: MIT
// SuperCapes settings card: Motion, Sources, About. Pause overlay or mod screen.
// Drawn in `screen_render_post` with the typed draw context.

use crate::logic as rules;
use crate::session as hook;
use mod_api::{ClientHost, DrawContext, ScreenView};
use rules::layout;
use rules::physics::Wind;
use rules::sources;

const SHADE: u32 = 0xA0081014;
const CARD: u32 = 0xFF0F1118;
const TAB: u32 = 0xFF161A22;
const CHIP: u32 = 0xFF1E2330;
const TEXT: u32 = 0xFFE8DCC8;
const DIM: u32 = 0xFF8B93A7;
const HOVER: u32 = 0xFF2A3142;

/// The live accent (`accent` setting, `0xRRGGBB`; Sunrise orange by default), set each paint.
static ACCENT_RGB: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0xF9801D);
/// Takes the accent colour the card draws with (`0xRRGGBB`).
pub fn set_accent(rgb: u32) {
    ACCENT_RGB.store(rgb & 0xFF_FFFF, std::sync::atomic::Ordering::Relaxed);
}
/// The accent, opaque: titles, values, the 2 px top edge.
fn accent() -> u32 {
    0xFF00_0000 | ACCENT_RGB.load(std::sync::atomic::Ordering::Relaxed)
}
/// The card's top edge: the accent.
fn edge() -> u32 {
    accent()
}
/// The selected tab / On chip: the accent darkened into the card.
fn tab_on() -> u32 {
    let a = accent();
    let ch = |s: u32| {
        let x = ((a >> s) & 0xFF) as f32;
        let b = ((CARD >> s) & 0xFF) as f32;
        ((x * 0.42 + b * 0.58).round() as u32).min(255) << s
    };
    0xFF00_0000 | ch(16) | ch(8) | ch(0)
}

fn hit(p: (f32, f32), r: [f32; 4]) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

fn fill(d: &mut dyn DrawContext, r: [f32; 4], c: u32) {
    d.fill(r, c);
}

fn ink(d: &mut dyn DrawContext, text: &str, pos: [f32; 2], c: u32) {
    d.text(text, pos, c, true);
}

fn ink_center(d: &mut dyn DrawContext, text: &str, cx: f32, y: f32, c: u32) {
    let w = d.text_width(text);
    ink(d, text, [cx - w * 0.5, y], c);
}

/// `text` cut with "..." to fit `max_w` (font width, plus the shadow pixel).
fn fit(d: &mut dyn DrawContext, text: &str, max_w: f32) -> String {
    let cell = std::cell::RefCell::new(d);
    let width = |t: &str| cell.borrow_mut().text_width(t) + 1.0;
    rules::text::fit(text, max_w, &width)
}

/// `text` word-wrapped to `max_w`.
fn wrap(d: &mut dyn DrawContext, text: &str, max_w: f32) -> Vec<String> {
    let cell = std::cell::RefCell::new(d);
    let width = |t: &str| cell.borrow_mut().text_width(t) + 1.0;
    rules::text::wrap(text, max_w, &width)
}

fn chip(d: &mut dyn DrawContext, r: [f32; 4], label: &str, on: bool, hover: bool) {
    let bg = if on {
        tab_on()
    } else if hover {
        HOVER
    } else {
        CHIP
    };
    fill(d, r, bg);
    let label = fit(d, label, r[2] - 6.0);
    ink_center(d, &label, r[0] + r[2] * 0.5, r[1] + 3.0, TEXT);
}

fn ours_mod(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    host.mod_screen() == Some(hook::MOD_ID) && screen.id == "mod"
}

fn ours_overlay(screen: &ScreenView<'_>) -> bool {
    screen.id == "paused" && hook::revise(|s| s.overlay)
}

fn card_open(host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
    ours_mod(host, screen) || ours_overlay(screen)
}

fn ui(screen: &ScreenView<'_>) -> rules::layout::Layout {
    layout::compute(screen.rect[2].max(1.0), screen.rect[3].max(1.0), rules::TABS.len())
}

pub fn paint(host: &dyn ClientHost, d: &mut dyn DrawContext, screen: &ScreenView<'_>) {
    if !hook::revise(|s| s.ready) {
        return;
    }
    if !card_open(host, screen) {
        return;
    }
    let lay = ui(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    fill(d, [0.0, 0.0, screen.rect[2], screen.rect[3]], SHADE);
    fill(d, lay.panel, CARD);
    fill(
        d,
        [lay.panel[0], lay.panel[1], lay.panel[2], 2.0],
        edge(),
    );
    ink(
        d,
        hook::TITLE,
        [lay.title[0], lay.title[1] + 4.0],
        accent(),
    );
    chip(d, lay.done, "Done", hit(p, lay.done), hit(p, lay.done));
    let tab = hook::revise(|s| s.tab);
    for (i, label) in rules::TABS.iter().enumerate() {
        let r = lay.tab_rect(i);
        let on = tab.index() == i;
        fill(d, r, if on { tab_on() } else { TAB });
        let label = fit(d, label, r[2] - 4.0);
        ink_center(
            d,
            &label,
            r[0] + r[2] * 0.5,
            r[1] + 4.0,
            if on { TEXT } else { DIM },
        );
    }
    match tab {
        hook::Tab::Motion => paint_motion(d, &lay, p),
        hook::Tab::Sources => paint_sources(d, &lay, p),
        hook::Tab::About => paint_about(d, &lay),
    }
}

/// The Motion tab's rows: (id, label, lit). Style picks Smooth (the
/// segmented cloth) or Blocky (the rigid vanilla cape); ripple, quality and
/// weight only shape the cloth, so Blocky hides them.
fn motion_rows(prefs: &rules::Prefs) -> Vec<(&'static str, String, bool)> {
    let mut rows = vec![("style", format!("Style: {}", prefs.style.label()), true)];
    if prefs.cloth() {
        rows.extend([
            ("wind", prefs.wind.label().to_string(), prefs.wind == Wind::Waves),
            ("quality", format!("Quality: {}", prefs.quality.label()), true),
            ("gravity", format!("Weight: {}", prefs.gravity), true),
            ("wind_strength", format!("Ripple strength: {}", prefs.wind_strength), true),
        ]);
    }
    rows.push((
        "elytra",
        format!("Elytra uses cape: {}", if prefs.elytra { "On" } else { "Off" }),
        prefs.elytra,
    ));
    rows
}

fn paint_motion(
    d: &mut dyn DrawContext,
    lay: &layout::Layout,
    p: (f32, f32),
) {
    let prefs = hook::prefs_snapshot();
    let rows = motion_rows(&prefs);
    for (i, (id, label, on)) in rows.iter().enumerate() {
        let r = layout::chip_row(lay.body, i);
        let _ = id;
        chip(d, r, label, *on, hit(p, r));
    }
}

fn paint_sources(
    d: &mut dyn DrawContext,
    lay: &layout::Layout,
    p: (f32, f32),
) {
    let (online, only_self, animated, sources_on, scroll) = hook::revise(|s| {
        (
            s.prefs.online,
            s.prefs.only_self,
            s.prefs.animated,
            s.prefs.sources.clone(),
            s.scroll,
        )
    });
    let head0 = layout::chip_row(lay.body, 0);
    let head1 = layout::chip_row(lay.body, 1);
    let head2 = layout::chip_row(lay.body, 2);
    chip(
        d,
        head0,
        &format!("Online capes: {}", if online { "On" } else { "Off" }),
        online,
        hit(p, head0),
    );
    chip(
        d,
        head1,
        &format!("Only me: {}", if only_self { "On" } else { "Off" }),
        only_self,
        hit(p, head1),
    );
    chip(
        d,
        head2,
        &format!("Animated capes: {}", if animated { "On" } else { "Off" }),
        animated,
        hit(p, head2),
    );
    let list_y = head2[1] + head2[3] + 4.0;
    let list = [
        lay.body[0],
        list_y,
        lay.body[2],
        (lay.body[1] + lay.body[3] - list_y).max(8.0),
    ];
    let mut paint_rows = |d: &mut dyn DrawContext| {
            for (i, (id, on)) in sources_on.iter().enumerate() {
                let Some(src) = sources::by_id(id) else {
                    continue;
                };
                let Some(row) = layout::source_row(list, scroll, i) else {
                    continue;
                };
                fill(d, row, if hit(p, row) { HOVER } else { CHIP });
                let mid = row[1] + 3.0;
                let tw = 28.0;
                let title = fit(d, src.title, row[2] - tw * 3.0 - 14.0);
                ink(
                    d,
                    &title,
                    [row[0] + 4.0, mid],
                    if *on { TEXT } else { DIM },
                );
                let up = [row[0] + row[2] - tw * 3.0 - 6.0, row[1] + 1.0, tw, row[3] - 2.0];
                let dn = [row[0] + row[2] - tw * 2.0 - 4.0, row[1] + 1.0, tw, row[3] - 2.0];
                let tog = [row[0] + row[2] - tw - 2.0, row[1] + 1.0, tw, row[3] - 2.0];
                fill(d, up, if hit(p, up) { tab_on() } else { TAB });
                fill(d, dn, if hit(p, dn) { tab_on() } else { TAB });
                fill(
                    d,
                    tog,
                    if *on { tab_on() } else { CHIP },
                );
                ink_center(d, "Up", up[0] + up[2] * 0.5, mid, TEXT);
                ink_center(d, "Dn", dn[0] + dn[2] * 0.5, mid, TEXT);
                ink_center(
                    d,
                    if *on { "On" } else { "Off" },
                    tog[0] + tog[2] * 0.5,
                    mid,
                    TEXT,
                );
            }
    };
    if d.with_scissor(list, &mut paint_rows).is_err() {
        paint_rows(d);
    }
}

fn paint_about(d: &mut dyn DrawContext, lay: &layout::Layout) {
    let lines = [
        format!("{} {}", hook::TITLE, hook::VERSION),
        hook::BLURB.to_string(),
        "Written in Rust for Sunrise. Not a Java or Fabric mod.".to_string(),
        "Independent implementation. Does not copy other cape mods.".to_string(),
        "Online sources are only asked for public cape pictures.".to_string(),
        "Nothing else is sent. There is no keybind.".to_string(),
        "Style: Smooth is the cloth cape, Blocky the vanilla one.".to_string(),
        "Open it with the SuperCapes button above Save and Quit, or /supercapes.".to_string(),
        hook::revise(|s| {
            if s.status.is_empty() {
                format!("Author: {}", hook::AUTHOR)
            } else {
                s.status.clone()
            }
        }),
    ];
    // Every line word-wrapped to the card's inner width; lines that don't
    // fit below are left out rather than drawn past the card.
    let max_w = lay.body[2] - 4.0;
    let bottom = lay.body[1] + lay.body[3];
    let mut y = lay.body[1];
    'lines: for line in lines {
        for part in wrap(d, &line, max_w) {
            if y + 9.0 > bottom {
                break 'lines;
            }
            ink(d, &part, [lay.body[0], y], TEXT);
            y += 11.0;
        }
        y += 3.0;
    }
}

pub fn click(host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> bool {
    if button != 0 || !hook::revise(|s| s.ready) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    if !card_open(host, screen) {
        return false;
    }
    let lay = ui(screen);
    if hit(p, lay.done) || !hit(p, lay.panel) {
        hook::request_close();
        return true;
    }
    let tab = hook::revise(|s| s.tab);
    for i in 0..rules::TABS.len() {
        if hit(p, lay.tab_rect(i)) {
            hook::revise(|s| {
                s.tab = hook::Tab::from_index(i);
                s.scroll = 0.0;
            });
            return true;
        }
    }
    match tab {
        hook::Tab::Motion => click_motion(&lay, p),
        hook::Tab::Sources => click_sources(&lay, p),
        hook::Tab::About => false,
    }
}

fn click_motion(lay: &layout::Layout, p: (f32, f32)) -> bool {
    let labels: Vec<&str> = motion_rows(&hook::prefs_snapshot())
        .into_iter()
        .map(|(id, ..)| id)
        .collect();
    for (i, id) in labels.iter().enumerate() {
        let r = layout::chip_row(lay.body, i);
        if !hit(p, r) {
            continue;
        }
        hook::revise(|s| match *id {
            "style" => s.prefs.style = s.prefs.style.next(),
            "wind" => s.prefs.wind = s.prefs.wind.next(),
            "quality" => s.prefs.quality = s.prefs.quality.next(),
            "gravity" => s.prefs.gravity = (s.prefs.gravity + 1) % 10,
            "wind_strength" => s.prefs.wind_strength = (s.prefs.wind_strength + 1) % 10,
            "elytra" => s.prefs.elytra = !s.prefs.elytra,
            _ => {}
        });
        hook::mark_prefs_dirty();
        return true;
    }
    false
}

fn click_sources(lay: &layout::Layout, p: (f32, f32)) -> bool {
    let head0 = layout::chip_row(lay.body, 0);
    let head1 = layout::chip_row(lay.body, 1);
    let head2 = layout::chip_row(lay.body, 2);
    if hit(p, head0) {
        hook::revise(|s| s.prefs.online = !s.prefs.online);
        hook::mark_prefs_dirty();
        return true;
    }
    if hit(p, head1) {
        hook::revise(|s| s.prefs.only_self = !s.prefs.only_self);
        hook::mark_prefs_dirty();
        return true;
    }
    if hit(p, head2) {
        hook::revise(|s| s.prefs.animated = !s.prefs.animated);
        hook::mark_prefs_dirty();
        return true;
    }
    let list_y = head2[1] + head2[3] + 4.0;
    let list = [
        lay.body[0],
        list_y,
        lay.body[2],
        (lay.body[1] + lay.body[3] - list_y).max(8.0),
    ];
    let (sources_on, scroll) = hook::revise(|s| (s.prefs.sources.clone(), s.scroll));
    for (i, (id, _)) in sources_on.iter().enumerate() {
        let Some(row) = layout::source_row(list, scroll, i) else {
            continue;
        };
        if !hit(p, row) {
            continue;
        }
        let tw = 28.0;
        let up = [row[0] + row[2] - tw * 3.0 - 6.0, row[1] + 1.0, tw, row[3] - 2.0];
        let dn = [row[0] + row[2] - tw * 2.0 - 4.0, row[1] + 1.0, tw, row[3] - 2.0];
        let tog = [row[0] + row[2] - tw - 2.0, row[1] + 1.0, tw, row[3] - 2.0];
        hook::revise(|s| {
            if hit(p, up) {
                sources::move_source(&mut s.prefs.sources, id, true);
            } else if hit(p, dn) {
                sources::move_source(&mut s.prefs.sources, id, false);
            } else if hit(p, tog) || hit(p, row) {
                if let Some((_, on)) = s.prefs.sources.iter_mut().find(|(x, _)| x == id) {
                    *on = !*on;
                }
            }
        });
        hook::mark_prefs_dirty();
        return true;
    }
    false
}

pub fn wheel(host: &dyn ClientHost, screen: &ScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !card_open(host, screen) {
        return false;
    }
    let lay = ui(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    if hook::revise(|s| s.tab) != hook::Tab::Sources || !hit(p, lay.body) {
        return false;
    }
    hook::revise(|s| {
        let n = s.prefs.sources.len() as f32;
        let max = (n * layout::ROW - lay.body[3] * 0.5).max(0.0);
        s.scroll = (s.scroll - scroll[1] as f32 * 12.0).clamp(0.0, max);
    });
    true
}

pub fn key(host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str) -> bool {
    if !card_open(host, screen) {
        return false;
    }
    if matches!(key, "key.keyboard.escape" | "escape" | "Escape") {
        hook::request_close();
        return true;
    }
    false
}
