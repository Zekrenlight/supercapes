// SPDX-License-Identifier: MIT
// SuperCapes's settings card: the page Sunrise opens for the mod's settings (SunnyMods' Settings
// button, Sunrise's own Mods list, or the mod's settings key all land on it), drawn in the
// shared Sunrise dark card with a live accent colour. Every control writes straight to the
// mod's registered settings, so values save and apply the moment they change.
use super::Game;
use std::cell::RefCell;
use sunrise_client::Result;
use sunrise_client::hud::HudBatch;
use sunrise_client::mod_list::NativeSettingValue;
use sunrise_client::options_ui::{Event, Page};
use sunrise_client::screen_draw::{self as d, Draw, Measure, TextField};
use sunrise_client::text::Text;
use winit::keyboard::KeyCode;

/// The mod this card belongs to (`id` in `sunrise-mod.toml`).
const MOD_ID: &str = "supercapes";
/// The card's title.
const TITLE: &str = "SuperCapes";
/// Tabs: a label and the setting keys under it. Keys not listed go under the first tab.
const TABS: &[(&str, &[&str])] = &[];
/// A line under the rows (where the rest of the mod's controls live), if any.
const NOTE: &str = "Cape settings: Pause menu > SuperCapes, or /supercapes.";

/// Sunrise orange, the default accent.
pub const DEFAULT_ACCENT: u32 = 0xF9801D;
/// The accent swatches: Sunrise orange first.
const PRESETS: [u32; 8] = [0xF9801D, 0xFED83D, 0x80C71F, 0x169C9C, 0x3AB3DA, 0x8932B8, 0xF38BAA, 0xB02E26];

// The shared Sunrise dark palette (ARGB).
const SHADE: u32 = 0xA008_1014;
const CARD: u32 = 0xFF0F_1118;
const SIDE: u32 = 0xFF1E_2330;
const TAB: u32 = 0xFF16_1A22;
const CHIP: u32 = 0xFF1E_2330;
const CHIP_EDGE: u32 = 0xFF2E_3546;
const CHIP_HOVER: u32 = 0xFF2A_3142;
const TRACK: u32 = 0xFF0A_0C11;
const TEXT: u32 = 0xFFE8_DCC8;
const DIM: u32 = 0xFF8B_93A7;
const ROW_HOVER: u32 = 0x14FF_FFFF;

const PAD: i32 = 10;
const ROW: i32 = 22;
const CHIP_H: i32 = 16;
const TAB_H: i32 = 18;
const BAR: i32 = 6;
const WHEEL: f64 = 22.0;

fn argb(c: u32) -> i32 {
    c as i32
}
/// An accent `0xRRGGBB` as opaque ARGB.
pub fn opaque(rgb: u32) -> u32 {
    0xFF00_0000 | (rgb & 0xFF_FFFF)
}
/// `rgb` mixed toward `base` (`t` of `rgb`), opaque.
fn mix(rgb: u32, base: u32, t: f32) -> u32 {
    let ch = |c: u32, s: u32| {
        let a = ((c >> s) & 0xFF) as f32;
        let b = ((base >> s) & 0xFF) as f32;
        ((a * t + b * (1.0 - t)).round() as u32).min(255) << s
    };
    0xFF00_0000 | ch(rgb, 16) | ch(rgb, 8) | ch(rgb, 0)
}
/// The selected-tab / On-chip fill: the accent darkened into the card.
pub fn tab_on(accent: u32) -> u32 {
    mix(accent, CARD, 0.42)
}
fn inside(r: [i32; 4], m: (i32, i32)) -> bool {
    d::inside(m, r[0], r[1], r[2], r[3])
}
fn lit(s: impl Into<String>) -> Text {
    Text::literal(s.into())
}
/// A translation key's text, or a readable fallback made from the key's last part.
fn tr_key(key: &str) -> Text {
    let last = key.rsplit('.').next().unwrap_or(key);
    let mut fallback = last.replace('_', " ");
    if let Some(first) = fallback.get(0..1) {
        fallback = first.to_uppercase() + &fallback[1..];
    }
    Text::translate_with_fallback(key, &fallback, &[])
}

/// The mod's row in Sunrise's installed list.
fn own_index(game: &Game<'_>) -> Option<usize> {
    game.settings.mods.installed.iter().position(|m| m.id == MOD_ID)
}

/// The accent colour (`0xRRGGBB`) mod `id` has set: its `accent` colour setting, live.
pub fn accent_of(game: &Game<'_>, id: &str) -> u32 {
    game.settings
        .mods
        .installed
        .iter()
        .find(|m| m.id == id)
        .and_then(|m| m.native.as_ref())
        .and_then(|page| page.settings.iter().find(|s| s.key == "accent"))
        .and_then(|s| match s.value {
            NativeSettingValue::Color(rgb) => Some(rgb & 0xFF_FFFF),
            _ => None,
        })
        .unwrap_or(DEFAULT_ACCENT)
}
/// This mod's accent colour, `0xRRGGBB`.
pub fn accent(game: &Game<'_>) -> u32 {
    accent_of(game, MOD_ID)
}

#[derive(Clone, Copy, PartialEq)]
enum Held {
    Bar,
    Slider(usize),
}

struct State {
    tab: usize,
    scroll: f64,
    held: Option<Held>,
    /// Sunrise's own page asked for (to rebind keys): it shows until the page is left.
    classic: bool,
    /// On this mod's page last frame.
    on_page: bool,
    search: TextField,
}

impl Default for State {
    fn default() -> Self {
        let mut search = TextField::new(0, 0, 100, CHIP_H);
        search.bordered = false;
        search.max_length = 40;
        search.hint = Some(lit("Search settings"));
        Self { tab: 0, scroll: 0.0, held: None, classic: false, on_page: false, search }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// The card shows on this frame: the Options screen is on this mod's settings page and
/// Sunrise's own page wasn't asked for. Resets the card when the page is freshly opened.
pub(super) fn active(game: &Game<'_>) -> bool {
    let mine = game.screen == super::Screen::Options
        && matches!(game.ui.options.page(), Page::NativeMod(i) if Some(i) == own_index(game));
    with(|s| {
        if !mine {
            s.on_page = false;
            s.classic = false;
            s.held = None;
            s.search.focused = false;
            return false;
        }
        if !s.on_page {
            s.on_page = true;
            s.tab = 0;
            s.scroll = 0.0;
            s.search.value.clear();
            s.search.cursor = 0;
            s.search.highlight = 0;
        }
        !s.classic
    })
    .unwrap_or(false)
}

/// One row of the card.
#[derive(Clone)]
enum Row {
    /// Setting `j` on the mod's page.
    Setting(usize),
    /// The mod's keys: a click opens Sunrise's page, where they are rebound.
    Keys,
}

/// The card's places at one GUI size.
struct Geo {
    card: [i32; 4],
    done: [i32; 4],
    tabs: Vec<[i32; 4]>,
    search: Option<[i32; 4]>,
    rows: [i32; 4],
    foot: i32,
}

fn geo(m: &Measure, w: i32, h: i32, searchable: bool) -> Geo {
    let cw = ((w as f32 * 0.62) as i32).clamp(280, 460).min(w - 8).max(120);
    let ch = ((h as f32 * 0.72) as i32).max(150).min(h - 8).max(80);
    let x = (w - cw) / 2;
    let y = (h - ch) / 2;
    let dw = m.width(&lit("Done")) + 12;
    let done = [x + cw - PAD - dw, y + 6, dw, CHIP_H];
    let mut top = y + 28;
    let mut tabs = Vec::new();
    if TABS.len() > 1 {
        let n = TABS.len() as i32;
        let inner = cw - 2 * PAD;
        let tw = (inner - (n - 1) * 2) / n;
        for i in 0..n {
            tabs.push([x + PAD + i * (tw + 2), top, tw, TAB_H]);
        }
        top += TAB_H + 4;
    }
    let search = searchable.then(|| {
        let r = [x + PAD, top, cw - 2 * PAD, CHIP_H];
        top += CHIP_H + 4;
        r
    });
    let foot_h = if NOTE.is_empty() { 14 } else { 24 };
    let rows = [x + PAD, top, cw - 2 * PAD, (y + ch - foot_h - top).max(ROW)];
    Geo { card: [x, y, cw, ch], done, tabs, search, rows, foot: y + ch - foot_h + 2 }
}

/// The rows the current tab and search show.
fn rows(game: &Game<'_>, s: &State) -> Vec<Row> {
    let Some(i) = own_index(game) else { return Vec::new() };
    let Some(page) = game.settings.mods.installed[i].native.as_ref() else { return Vec::new() };
    let query = s.search.value.trim().to_lowercase();
    let tab_of = |key: &str| TABS.iter().position(|(_, keys)| keys.contains(&key)).unwrap_or(0);
    let mut out: Vec<Row> = page
        .settings
        .iter()
        .enumerate()
        .filter(|(_, set)| {
            if !query.is_empty() {
                let label = tr_key(&set.label).plain().to_lowercase();
                return label.contains(&query) || set.key.contains(&query);
            }
            TABS.len() <= 1 || tab_of(&set.key) == s.tab
        })
        .map(|(j, _)| Row::Setting(j))
        .collect();
    let last_tab = s.tab + 1 == TABS.len().max(1);
    if !page.mappings.is_empty() && (last_tab || !query.is_empty()) {
        out.push(Row::Keys);
    }
    out
}

fn content(n: usize) -> i32 {
    n as i32 * ROW
}
fn max_scroll(n: usize, view: i32) -> f64 {
    f64::from((content(n) - view).max(0))
}
fn thumb(n: usize, view: i32, scroll: f64) -> Option<(i32, i32)> {
    let total = content(n);
    if total <= view {
        return None;
    }
    let len = ((view as f64 * view as f64 / total as f64) as i32).max(12).min(view);
    let room = (view - len).max(1);
    let at = (scroll / max_scroll(n, view) * f64::from(room)) as i32;
    Some((at, len))
}

/// A control's places inside row `r`.
struct Controls {
    /// A toggle, a choice or the keys' chip.
    chip: [i32; 4],
    minus: [i32; 4],
    track: [i32; 4],
    plus: [i32; 4],
    swatches: Vec<([i32; 4], u32)>,
    reset: [i32; 4],
}

fn controls(r: [i32; 4]) -> Controls {
    let right = r[0] + r[2] - BAR - 6;
    let cy = r[1] + (ROW - CHIP_H) / 2;
    let chip_w = 92;
    let chip = [right - chip_w, cy, chip_w, CHIP_H];
    let plus = [right - CHIP_H, cy, CHIP_H, CHIP_H];
    let track = [plus[0] - 4 - 64, cy + 5, 64, 6];
    let minus = [track[0] - 4 - CHIP_H, cy, CHIP_H, CHIP_H];
    let sw = 12;
    let reset = [right - 30, cy, 30, CHIP_H];
    let start = reset[0] - 4 - PRESETS.len() as i32 * (sw + 2);
    let swatches = PRESETS
        .iter()
        .enumerate()
        .map(|(k, &c)| ([start + k as i32 * (sw + 2), cy + 2, sw, sw], c))
        .collect();
    Controls { chip, minus, track, plus, swatches, reset }
}

fn chip(out: &mut Vec<Draw>, m: &Measure, r: [i32; 4], label: Text, bg: u32, edge: u32, color: u32) {
    let [x, y, w, h] = r;
    d::fill(out, x, y, x + w, y + h, argb(edge));
    d::fill(out, x + 1, y + 1, x + w - 1, y + h - 1, argb(bg));
    let lw = m.width(&label);
    d::text(out, label, x + (w - lw) / 2, y + (h - 8) / 2, argb(color), false);
}

/// A key binding (`key.keyboard.o`) as its key's name.
fn key_name(key: &str) -> String {
    let last = key.rsplit('.').next().unwrap_or(key);
    if key.ends_with(".unknown") {
        return "Not bound".into();
    }
    if last.len() == 1 {
        return last.to_uppercase();
    }
    let mut s = last.replace('_', " ");
    if let Some(first) = s.get(0..1) {
        s = first.to_uppercase() + &s[1..];
    }
    s
}

fn range_text(value: f64, step: f64) -> String {
    if step >= 1.0 && value.fract().abs() < 1e-9 {
        format!("{}", value as i64)
    } else if step >= 0.1 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

/// Draws the card over the menu background, in place of Sunrise's settings page.
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    let (w, h) = (batch.width, batch.height);
    let in_world = game.ui.options.in_world;
    sunrise_client::ui::menu_background(batch, &game.gui.atlas, 0.0, 0.0, w, h, in_world);
    let mouse = (game.cursor.0.floor() as i32, game.cursor.1.floor() as i32);
    let accent = opaque(accent(game));
    let on_fill = tab_on(accent);
    let index = own_index(game);
    let draws = {
        let game: &Game<'_> = game;
        let atlas = &game.gui.atlas;
        let m = Measure(&atlas.font);
        with(|s| {
            let count = index
                .and_then(|i| game.settings.mods.installed[i].native.as_ref())
                .map_or(0, |p| p.settings.len());
            let g = geo(&m, w as i32, h as i32, count > 10);
            let list = rows(game, s);
            s.scroll = s.scroll.clamp(0.0, max_scroll(list.len(), g.rows[3]));
            let mut out = Vec::new();
            let [x, y, cw, ch] = g.card;
            d::fill(&mut out, 0, 0, w as i32, h as i32, argb(SHADE));
            d::fill(&mut out, x - 1, y, x + cw + 1, y + ch + 1, argb(SIDE));
            d::fill(&mut out, x, y, x + cw, y + ch, argb(CARD));
            d::fill(&mut out, x - 1, y, x + cw + 1, y + 2, argb(accent));
            d::text(&mut out, lit(TITLE).bold(), x + 12, y + 8, argb(accent), false);
            let done_hover = inside(g.done, mouse);
            chip(&mut out, &m, g.done, lit("Done"), if done_hover { CHIP_HOVER } else { CHIP }, CHIP_EDGE, TEXT);
            for (t, r) in g.tabs.iter().enumerate() {
                let on = t == s.tab && s.search.value.trim().is_empty();
                let hover = inside(*r, mouse);
                let bg = if on { on_fill } else if hover { CHIP_HOVER } else { TAB };
                d::fill(&mut out, r[0], r[1], r[0] + r[2], r[1] + r[3], argb(bg));
                if on {
                    d::fill(&mut out, r[0], r[1] + r[3] - 1, r[0] + r[2], r[1] + r[3], argb(accent));
                }
                let label = d::clip(&m, &lit(TABS[t].0), r[2] - 6);
                let lw = m.width(&label);
                d::text(&mut out, label, r[0] + (r[2] - lw) / 2, r[1] + 5, argb(if on { TEXT } else { DIM }), false);
            }
            if let Some(r) = g.search {
                d::fill(&mut out, r[0], r[1], r[0] + r[2], r[1] + r[3], argb(if s.search.focused { accent } else { CHIP_EDGE }));
                d::fill(&mut out, r[0] + 1, r[1] + 1, r[0] + r[2] - 1, r[1] + r[3] - 1, argb(TRACK));
                s.search.x = r[0] + 4;
                s.search.y = r[1] + 4;
                s.search.width = r[2] - 8;
                s.search.height = r[3] - 8;
                s.search.draw(&mut out, &m, 0, None);
            }
            let page = index.and_then(|i| game.settings.mods.installed[i].native.as_ref());
            let [rx, ry, rw, rh] = g.rows;
            let mut tip: Option<Text> = None;
            for (n, row) in list.iter().enumerate() {
                let top = ry + n as i32 * ROW - s.scroll as i32;
                if top + ROW <= ry || top >= ry + rh {
                    continue;
                }
                let r = [rx, top, rw, ROW];
                let hover = inside(r, mouse) && mouse.1 >= ry && mouse.1 < ry + rh;
                if hover {
                    d::fill(&mut out, rx, top.max(ry), rx + rw - BAR - 2, (top + ROW).min(ry + rh), argb(ROW_HOVER));
                }
                let c = controls(r);
                match row {
                    Row::Keys => {
                        let keys = page.map_or(0, |p| p.mappings.len());
                        d::text(&mut out, lit(format!("Key binds ({keys})")), rx + 4, top + 7, argb(TEXT), false);
                        let ch_hover = inside(c.chip, mouse);
                        chip(&mut out, &m, c.chip, lit("Rebind \u{203A}"), if ch_hover { CHIP_HOVER } else { CHIP }, CHIP_EDGE, TEXT);
                        if hover {
                            let names: Vec<String> = page
                                .map(|p| {
                                    p.mappings
                                        .iter()
                                        .map(|k| {
                                            let bound = game
                                                .settings
                                                .mods
                                                .keys
                                                .get(MOD_ID)
                                                .and_then(|keys| keys.get(&k.id))
                                                .unwrap_or(&k.key);
                                            format!("{}: {}", tr_key(&k.label).plain(), key_name(bound))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            tip = Some(lit(names.join(" \u{00B7} ")));
                        }
                    }
                    Row::Setting(j) => {
                        let Some(set) = page.and_then(|p| p.settings.get(*j)) else { continue };
                        let label = tr_key(&set.label);
                        let label_room = match &set.value {
                            NativeSettingValue::Color(_) => c.swatches[0].0[0] - rx - 8,
                            NativeSettingValue::Range { .. } => c.minus[0] - rx - 8 - 30,
                            _ => c.chip[0] - rx - 8,
                        };
                        d::text(&mut out, d::clip(&m, &label, label_room.max(20)), rx + 4, top + 7, argb(TEXT), false);
                        if hover && let Some(t) = &set.tooltip {
                            tip = Some(Text::translate_with_fallback(t, "", &[]));
                        }
                        match &set.value {
                            NativeSettingValue::Toggle(on) => {
                                let ch_hover = inside(c.chip, mouse);
                                let (bg, edge, color) = if *on {
                                    (on_fill, accent, TEXT)
                                } else {
                                    (if ch_hover { CHIP_HOVER } else { CHIP }, CHIP_EDGE, DIM)
                                };
                                chip(&mut out, &m, c.chip, lit(if *on { "On" } else { "Off" }), bg, edge, color);
                            }
                            NativeSettingValue::Choice { options, index } => {
                                let ch_hover = inside(c.chip, mouse);
                                let name = options.get(*index).map_or_else(|| lit("?"), |o| tr_key(o));
                                let name = d::clip(&m, &name, c.chip[2] - 18);
                                let label = lit("\u{2039} ").append(name).append(lit(" \u{203A}"));
                                chip(&mut out, &m, c.chip, label, if ch_hover { CHIP_HOVER } else { CHIP }, CHIP_EDGE, TEXT);
                            }
                            NativeSettingValue::Range { step, value, .. } => {
                                let v = lit(range_text(*value, *step));
                                let vw = m.width(&v);
                                d::text(&mut out, v, c.minus[0] - 6 - vw, top + 7, argb(accent), false);
                                for (r, sign) in [(c.minus, "-"), (c.plus, "+")] {
                                    let hv = inside(r, mouse);
                                    chip(&mut out, &m, r, lit(sign), if hv { CHIP_HOVER } else { CHIP }, CHIP_EDGE, TEXT);
                                }
                                let t = c.track;
                                d::fill(&mut out, t[0], t[1], t[0] + t[2], t[1] + t[3], argb(TRACK));
                                let fill = (f64::from(set.value.fraction()) * f64::from(t[2])) as i32;
                                d::fill(&mut out, t[0], t[1], t[0] + fill, t[1] + t[3], argb(accent));
                                let kx = (t[0] + fill - 2).clamp(t[0], t[0] + t[2] - 4);
                                d::fill(&mut out, kx, t[1] - 3, kx + 4, t[1] + t[3] + 3, argb(TEXT));
                            }
                            NativeSettingValue::Color(rgb) => {
                                for (r, col) in &c.swatches {
                                    let picked = (*rgb & 0xFF_FFFF) == *col;
                                    let hv = inside(*r, mouse);
                                    let edge = if picked { TEXT } else if hv { DIM } else { CHIP_EDGE };
                                    d::fill(&mut out, r[0] - 1, r[1] - 1, r[0] + r[2] + 1, r[1] + r[3] + 1, argb(edge));
                                    d::fill(&mut out, r[0], r[1], r[0] + r[2], r[1] + r[3], argb(opaque(*col)));
                                }
                                // The current colour (right-click steps through all sixteen dye colours).
                                let cur = c.reset;
                                let is_accent = set.key == "accent";
                                let hv = inside(cur, mouse);
                                if is_accent {
                                    chip(&mut out, &m, cur, lit("Reset"), if hv { CHIP_HOVER } else { CHIP }, CHIP_EDGE, if *rgb & 0xFF_FFFF == DEFAULT_ACCENT { DIM } else { TEXT });
                                    if hv {
                                        tip = Some(lit(format!("Back to Sunrise orange #F9801D (now #{:06X})", rgb & 0xFF_FFFF)));
                                    }
                                } else {
                                    d::fill(&mut out, cur[0], cur[1], cur[0] + cur[2], cur[1] + cur[3], argb(if hv { TEXT } else { CHIP_EDGE }));
                                    d::fill(&mut out, cur[0] + 1, cur[1] + 1, cur[0] + cur[2] - 1, cur[1] + cur[3] - 1, argb(opaque(*rgb)));
                                    if hv {
                                        tip = Some(lit(format!("#{:06X}: click for the next dye colour", rgb & 0xFF_FFFF)));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if list.is_empty() {
                let t = lit(if s.search.value.trim().is_empty() { "Nothing to set here." } else { "No setting matches." });
                d::text(&mut out, t, rx + 4, ry + 6, argb(DIM), false);
            }
            // The scrollbar.
            if let Some((at, len)) = thumb(list.len(), rh, s.scroll) {
                let bx = rx + rw - BAR;
                d::fill(&mut out, bx, ry, bx + BAR, ry + rh, argb(TRACK));
                let held = s.held == Some(Held::Bar);
                d::fill(&mut out, bx, ry + at, bx + BAR, ry + at + len, argb(if held { accent } else { CHIP_EDGE }));
            }
            if !NOTE.is_empty() {
                d::text(&mut out, d::clip(&m, &lit(NOTE), cw - 2 * PAD), x + PAD, g.foot, argb(DIM), false);
            }
            if let Some(t) = tip.filter(|t| !t.plain().is_empty()) {
                d::tooltip(&mut out, &m, &t, mouse);
            }
            out
        })
        .unwrap_or_default()
    };
    let atlas = &game.gui.atlas;
    let tooltip = d::paint(batch, atlas, &draws);
    d::paint_tooltip(batch, atlas, &draws, tooltip);
}

/// What a click asks of the game, done once the card's state is let go.
enum Act {
    None,
    Click,
    /// Setting `j` changed to this value.
    Set(usize, NativeSettingValue),
    Classic,
    Done,
}

fn setting_value(game: &Game<'_>, j: usize) -> Option<NativeSettingValue> {
    let i = own_index(game)?;
    game.settings.mods.installed[i].native.as_ref()?.settings.get(j).map(|s| s.value.clone())
}

/// Range setting `j` at slider place `x`.
fn slide_to(game: &Game<'_>, j: usize, track: [i32; 4], x: f64) -> Act {
    let Some(mut v) = setting_value(game, j) else { return Act::None };
    let before = v.clone();
    let f = ((x - f64::from(track[0])) / f64::from(track[2].max(1))).clamp(0.0, 1.0);
    v.slide(f as f32);
    if v == before { Act::None } else { Act::Set(j, v) }
}

fn step_range(v: &mut NativeSettingValue, dir: f64) {
    if let NativeSettingValue::Range { min, max, step, value } = v {
        *value = (*value + dir * *step).clamp(*min, *max);
        let steps = ((*value - *min) / *step).round();
        *value = (*min + steps * *step).clamp(*min, *max);
    }
}

/// A mouse button on the card.
pub(super) fn mouse(game: &mut Game<'_>, pressed: bool, right: bool) -> Result<()> {
    if !pressed {
        with(|s| s.held = None);
        return Ok(());
    }
    let (w, h) = game.gui_size();
    let mouse = (game.cursor.0.floor() as i32, game.cursor.1.floor() as i32);
    let (fx, fy) = (f64::from(game.cursor.0), f64::from(game.cursor.1));
    let act = {
        let game: &Game<'_> = game;
        let atlas = &game.gui.atlas;
        let m = Measure(&atlas.font);
        let count = own_index(game)
            .and_then(|i| game.settings.mods.installed[i].native.as_ref())
            .map_or(0, |p| p.settings.len());
        with(|s| {
            let g = geo(&m, w as i32, h as i32, count > 10);
            let list = rows(game, s);
            s.search.focused = false;
            if right {
                // Right-click steps a choice back and a colour through the dye colours.
                let [rx, ry, rw, rh] = g.rows;
                if inside(g.rows, mouse) {
                    let n = ((fy - f64::from(ry) + s.scroll) / f64::from(ROW)).floor();
                    if n >= 0.0 && let Some(Row::Setting(j)) = list.get(n as usize) {
                        let top = ry + n as i32 * ROW - s.scroll as i32;
                        let c = controls([rx, top, rw, ROW]);
                        if let Some(mut v) = setting_value(game, *j) {
                            match &mut v {
                                NativeSettingValue::Choice { options, index } if inside(c.chip, mouse) => {
                                    let len = options.len().max(1);
                                    *index = (*index + len - 1) % len;
                                    return Act::Set(*j, v);
                                }
                                NativeSettingValue::Color(_) if inside(c.reset, mouse) => {
                                    v.click();
                                    return Act::Set(*j, v);
                                }
                                _ => {}
                            }
                        }
                    }
                    let _ = rh;
                }
                return Act::None;
            }
            if inside(g.done, mouse) {
                return Act::Done;
            }
            for (t, r) in g.tabs.iter().enumerate() {
                if inside(*r, mouse) {
                    s.tab = t;
                    s.scroll = 0.0;
                    s.search.value.clear();
                    s.search.cursor = 0;
                    s.search.highlight = 0;
                    return Act::Click;
                }
            }
            if let Some(r) = g.search
                && inside(r, mouse)
            {
                s.search.focused = true;
                s.search.click(&m, fx, false, false);
                return Act::None;
            }
            let [rx, ry, rw, rh] = g.rows;
            let track = [rx + rw - BAR, ry, BAR, rh];
            if inside(track, mouse) && thumb(list.len(), rh, s.scroll).is_some() {
                s.held = Some(Held::Bar);
                drag_bar(s, &g, list.len(), fy);
                return Act::None;
            }
            if !inside(g.rows, mouse) {
                return Act::None;
            }
            let n = ((fy - f64::from(ry) + s.scroll) / f64::from(ROW)).floor();
            if n < 0.0 {
                return Act::None;
            }
            let Some(row) = list.get(n as usize) else { return Act::None };
            let top = ry + n as i32 * ROW - s.scroll as i32;
            let c = controls([rx, top, rw, ROW]);
            match row {
                Row::Keys => {
                    if inside(c.chip, mouse) {
                        return Act::Classic;
                    }
                    Act::None
                }
                Row::Setting(j) => {
                    let j = *j;
                    let Some(mut v) = setting_value(game, j) else { return Act::None };
                    match &mut v {
                        NativeSettingValue::Toggle(_) | NativeSettingValue::Choice { .. } => {
                            // A click anywhere on the row flips a toggle; a choice moves on.
                            v.click();
                            Act::Set(j, v)
                        }
                        NativeSettingValue::Range { .. } => {
                            if inside(c.minus, mouse) {
                                step_range(&mut v, -1.0);
                                return Act::Set(j, v);
                            }
                            if inside(c.plus, mouse) {
                                step_range(&mut v, 1.0);
                                return Act::Set(j, v);
                            }
                            let grab = [c.track[0] - 2, top, c.track[2] + 4, ROW];
                            if inside(grab, mouse) {
                                s.held = Some(Held::Slider(j));
                                return slide_to(game, j, c.track, fx);
                            }
                            Act::None
                        }
                        NativeSettingValue::Color(rgb) => {
                            for (r, col) in &c.swatches {
                                if inside(*r, mouse) {
                                    *rgb = *col;
                                    return Act::Set(j, v);
                                }
                            }
                            if inside(c.reset, mouse) {
                                if s_is_accent(game, j) {
                                    *rgb = DEFAULT_ACCENT;
                                } else {
                                    v.click();
                                }
                                return Act::Set(j, v);
                            }
                            Act::None
                        }
                    }
                }
            }
        })
        .unwrap_or(Act::None)
    };
    perform(game, act)
}

fn s_is_accent(game: &Game<'_>, j: usize) -> bool {
    own_index(game)
        .and_then(|i| game.settings.mods.installed[i].native.as_ref())
        .and_then(|p| p.settings.get(j))
        .is_some_and(|s| s.key == "accent")
}

fn perform(game: &mut Game<'_>, act: Act) -> Result<()> {
    match act {
        Act::None => Ok(()),
        Act::Click => game.handle_option_events(vec![Event::ButtonClick]),
        Act::Set(j, value) => {
            let Some(i) = own_index(game) else { return Ok(()) };
            let changed = game.settings.mods.installed[i]
                .native
                .as_mut()
                .and_then(|p| p.settings.get_mut(j))
                .map(|s| {
                    let changed = s.value != value;
                    s.value = value;
                    changed
                })
                .unwrap_or(false);
            if !changed {
                return Ok(());
            }
            let dragging = with(|s| matches!(s.held, Some(Held::Slider(_)))).unwrap_or(false);
            let mut events = Vec::new();
            if !dragging {
                events.push(Event::ButtonClick);
            }
            events.push(Event::NativeSetting(i, j));
            game.handle_option_events(events)
        }
        Act::Classic => {
            with(|s| s.classic = true);
            game.handle_option_events(vec![Event::ButtonClick])
        }
        Act::Done => {
            game.handle_option_events(vec![Event::ButtonClick])?;
            with(|s| s.search.focused = false);
            // Leaves as the page's Escape does (back to the Mods list).
            game.options_key(KeyCode::Escape, None).map(|_| ())
        }
    }
}

fn drag_bar(s: &mut State, g: &Geo, n: usize, y: f64) {
    let rh = g.rows[3];
    let Some((_, len)) = thumb(n, rh, 0.0) else { return };
    let room = f64::from((rh - len).max(1));
    let at = (y - f64::from(len) / 2.0 - f64::from(g.rows[1])).clamp(0.0, room);
    s.scroll = at / room * max_scroll(n, rh);
}

/// The pointer moved with a button held: a held scrollbar or slider follows it.
pub(super) fn drag(game: &mut Game<'_>) -> Result<()> {
    let (w, h) = game.gui_size();
    let (fx, fy) = (f64::from(game.cursor.0), f64::from(game.cursor.1));
    let act = {
        let game: &Game<'_> = game;
        let atlas = &game.gui.atlas;
        let m = Measure(&atlas.font);
        let count = own_index(game)
            .and_then(|i| game.settings.mods.installed[i].native.as_ref())
            .map_or(0, |p| p.settings.len());
        with(|s| {
            let g = geo(&m, w as i32, h as i32, count > 10);
            let list = rows(game, s);
            match s.held {
                Some(Held::Bar) => {
                    drag_bar(s, &g, list.len(), fy);
                    Act::None
                }
                Some(Held::Slider(j)) => {
                    let Some(n) = list.iter().position(|r| matches!(r, Row::Setting(k) if *k == j)) else {
                        return Act::None;
                    };
                    let top = g.rows[1] + n as i32 * ROW - s.scroll as i32;
                    let c = controls([g.rows[0], top, g.rows[2], ROW]);
                    slide_to(game, j, c.track, fx)
                }
                None => Act::None,
            }
        })
        .unwrap_or(Act::None)
    };
    perform(game, act)
}

/// The wheel scrolls the rows.
pub(super) fn scroll(game: &mut Game<'_>, amount: f32) -> Result<()> {
    let (w, h) = game.gui_size();
    let game: &Game<'_> = game;
    let atlas = &game.gui.atlas;
    let m = Measure(&atlas.font);
    let count = own_index(game)
        .and_then(|i| game.settings.mods.installed[i].native.as_ref())
        .map_or(0, |p| p.settings.len());
    with(|s| {
        let g = geo(&m, w as i32, h as i32, count > 10);
        let n = rows(game, s).len();
        s.scroll = (s.scroll - f64::from(amount) * WHEEL).clamp(0.0, max_scroll(n, g.rows[3]));
    });
    Ok(())
}

/// A key on the card: `None` lets Sunrise have it (Escape, to leave).
pub(super) fn key(game: &mut Game<'_>, key: KeyCode, text: Option<&str>) -> Option<Result<()>> {
    let shift = game.has_shift_down();
    let ctrl = game.has_control_down();
    let tabs = TABS.len().max(1);
    let atlas = &game.gui.atlas;
    let m = Measure(&atlas.font);
    let handled = with(|s| {
        if key == KeyCode::Escape {
            if s.search.focused {
                s.search.focused = false;
                return true;
            }
            return false;
        }
        if s.search.focused {
            let before = s.search.value.clone();
            if s.search.key(&m, key, shift, ctrl).is_none()
                && let Some(t) = text.filter(|t| !t.chars().any(char::is_control))
            {
                s.search.insert_text(&m, t);
            }
            if s.search.value != before {
                s.scroll = 0.0;
            }
            return true;
        }
        match key {
            KeyCode::Tab if tabs > 1 => {
                s.tab = if shift { (s.tab + tabs - 1) % tabs } else { (s.tab + 1) % tabs };
                s.scroll = 0.0;
            }
            KeyCode::PageUp | KeyCode::ArrowUp => s.scroll = (s.scroll - WHEEL * 3.0).max(0.0),
            KeyCode::PageDown | KeyCode::ArrowDown => s.scroll += WHEEL * 3.0,
            KeyCode::Home => s.scroll = 0.0,
            KeyCode::KeyF if ctrl => s.search.focused = true,
            _ => return false,
        }
        true
    })
    .unwrap_or(false);
    if handled { Some(Ok(())) } else if key == KeyCode::Escape { None } else { Some(Ok(())) }
}
