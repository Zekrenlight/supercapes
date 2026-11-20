// SPDX-License-Identifier: MIT
// SuperCapes game side: a support module in the mirror of
// `crates/sunrise-client/src/game/native_hooks/render_frame_pre.rs`, called once a
// frame by the `frame` mixin. It reads and writes the prefs file, downloads and
// decodes public cape pictures, puts them in the atlas's skin slots, and sets
// each player's cape sprite. The state and the rules live in the mod's crate.

use crate::game::Game;
use mod_supercapes::logic as rules;
use mod_supercapes::session as hook;
use hook::{CapeFrame, FetchMsg, ReadyCape};
use rules::persist::{dump_prefs, load_prefs, FILE_NAME};
use rules::sources;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::time::{Duration, Instant};

fn data_dir(game: &Game<'_>) -> std::path::PathBuf {
    game.native_data_dir(hook::MOD_ID)
}

fn cache_dir(game: &Game<'_>) -> std::path::PathBuf {
    data_dir(game).join("cache")
}

fn ensure_ready(game: &Game<'_>) {
    if hook::revise(|s| s.ready) {
        return;
    }
    let dir = data_dir(game);
    let _ = fs::create_dir_all(dir.join("cache"));
    let text = fs::read_to_string(dir.join(FILE_NAME)).unwrap_or_default();
    let prefs = load_prefs(&text);
    hook::revise(|s| {
        s.prefs = prefs;
        s.ready = true;
    });
}

fn write_prefs_if_dirty(game: &Game<'_>) {
    let Some(text) = hook::revise(|s| {
        std::mem::take(&mut s.prefs_dirty).then(|| dump_prefs(&s.prefs))
    }) else {
        return;
    };
    let dir = data_dir(game);
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(FILE_NAME), text);
}

/// Once a frame, before the frame is drawn.
pub fn frame(game: &mut Game<'_>) {
    ensure_ready(game);
    write_prefs_if_dirty(game);
    let paused = game.screen == crate::game::Screen::Paused;
    // A preview interrupted mid-draw can't leave the flag on.
    hook::set_preview(false);
    hook::revise(|s| {
        if s.overlay && !paused {
            s.overlay = false;
            s.want_close = false;
        }
    });
    poll_fetches(game);
    maybe_fetch(game);
    apply_capes(game);
    note_bodies(game);
}

fn poll_fetches(game: &mut Game<'_>) {
    let msgs: Vec<FetchMsg> = hook::revise(|s| {
        let mut out = Vec::new();
        if let Some(rx) = s.rx.as_mut() {
            while let Ok(m) = rx.try_recv() {
                out.push(m);
            }
        }
        out
    });
    for m in msgs {
        match m {
            FetchMsg::Miss { uuid } => {
                hook::revise(|s| {
                    s.in_flight.remove(&uuid);
                });
            }
            FetchMsg::Pic {
                uuid,
                source,
                bytes,
                sheet,
                delay_ms,
            } => {
                hook::revise(|s| {
                    s.in_flight.remove(&uuid);
                });
                if let Some(cape) = process_image(&bytes, source, sheet, delay_ms) {
                    let dir = cache_dir(game);
                    let _ = fs::create_dir_all(&dir);
                    let _ = fs::write(dir.join(format!("{uuid:032x}.bin")), write_cache(&cape));
                    // A fetch started while the source was on can finish after
                    // it was turned off. Keep the cache, but do not show it.
                    let is_local = local_player_uuid(game) == Some(uuid);
                    let allowed = hook::revise(|s| s.prefs.provider_allowed(&cape.source, is_local));
                    if allowed {
                        for frame in &cape.frames {
                            game.store_skin(frame.location.clone(), frame.rgba.clone());
                        }
                        hook::revise(|s| {
                            s.capes.insert(uuid, cape);
                        });
                    }
                }
            }
        }
    }
}

// SC3: HD OptiFine capes are fitted to 46x22 before the slot (1.0.0);
// SC2 entries hold the old stretched pictures, so they are fetched again.
const CACHE_MAGIC: &[u8] = b"SC3\0";

fn write_cache(cape: &ReadyCape) -> Vec<u8> {
    let mut buf = CACHE_MAGIC.to_vec();
    buf.extend_from_slice(cape.source.as_bytes());
    buf.push(0);
    buf.extend_from_slice(&(cape.frames.len() as u16).to_le_bytes());
    for frame in &cape.frames {
        buf.extend_from_slice(frame.location.as_bytes());
        buf.push(0);
        buf.extend_from_slice(&frame.delay_ms.to_le_bytes());
        buf.extend_from_slice(&(frame.rgba.len() as u32).to_le_bytes());
        buf.extend_from_slice(&frame.rgba);
    }
    buf
}

fn load_cache(game: &Game<'_>, uuid: u128) -> Option<ReadyCape> {
    let bytes = fs::read(cache_dir(game).join(format!("{uuid:032x}.bin"))).ok()?;
    if !bytes.starts_with(CACHE_MAGIC) {
        return None;
    }
    read_cache_v2(&bytes[CACHE_MAGIC.len()..])
}

fn read_cache_v2(bytes: &[u8]) -> Option<ReadyCape> {
    let z = bytes.iter().position(|b| *b == 0)?;
    let source = String::from_utf8(bytes[..z].to_vec()).ok()?;
    let rest = &bytes[z + 1..];
    if rest.len() < 2 {
        return None;
    }
    let n = u16::from_le_bytes([rest[0], rest[1]]) as usize;
    if n == 0 || n > rules::image::MAX_FRAMES {
        return None;
    }
    let mut cur = &rest[2..];
    let mut frames = Vec::with_capacity(n);
    for _ in 0..n {
        let z = cur.iter().position(|b| *b == 0)?;
        let location = String::from_utf8(cur[..z].to_vec()).ok()?;
        cur = &cur[z + 1..];
        if cur.len() < 6 {
            return None;
        }
        let delay_ms = u16::from_le_bytes([cur[0], cur[1]]);
        let len = u32::from_le_bytes(cur[2..6].try_into().ok()?) as usize;
        cur = &cur[6..];
        if cur.len() < len || len != 64 * 64 * 4 {
            return None;
        }
        let rgba = cur[..len].to_vec();
        cur = &cur[len..];
        frames.push(CapeFrame {
            location,
            rgba,
            delay_ms,
        });
    }
    Some(ReadyCape { source, frames })
}

/// The local player's profile UUID, as `update_remote_skins` reads it.
fn local_player_uuid(game: &Game<'_>) -> Option<u128> {
    game.remote.as_ref().map(|r| r.profile)
}

fn maybe_fetch(game: &Game<'_>) {
    let local_uuid = local_player_uuid(game);
    hook::drop_disallowed_capes(local_uuid);
    let (online, only_self, sources_on) =
        hook::revise(|s| (s.prefs.online, s.prefs.only_self, s.prefs.sources.clone()));
    if !online {
        return;
    }
    let enabled = sources::enabled_in_order(&sources_on);
    if enabled.is_empty() {
        return;
    }
    let mut wanted: Vec<(u128, String)> = Vec::new();
    let local_name = game.options.player_name();
    if let Some(u) = local_player_uuid(game) {
        if !local_name.is_empty() {
            wanted.push((u, local_name));
        }
    }
    if !only_self {
        for (id, name) in game.remote_player_names() {
            if let Some(u) = game.remote_player_uuid(id) {
                if !wanted.iter().any(|(x, _)| *x == u) {
                    wanted.push((u, name));
                }
            }
        }
    }
    for (uuid, name) in wanted {
        if hook::revise(|s| s.capes.contains_key(&uuid)) {
            continue;
        }
        if let Some(cached) = load_cache(game, uuid) {
            let is_local = local_uuid == Some(uuid);
            let allowed = hook::revise(|s| s.prefs.provider_allowed(&cached.source, is_local));
            if allowed {
                hook::revise(|s| {
                    s.capes.insert(uuid, cached);
                });
                continue;
            }
            // Cache is from a source that's now off: try the others.
        }
        let busy = hook::revise(|s| {
            if let Some(t) = s.in_flight.get(&uuid) {
                if t.elapsed() < Duration::from_secs(30) {
                    return true;
                }
            }
            s.in_flight.insert(uuid, Instant::now());
            false
        });
        if busy {
            continue;
        }
        let Some(tx) = hook::revise(|s| s.tx.clone()) else {
            continue;
        };
        let jobs: Vec<(String, sources::Kind, String)> = enabled
            .iter()
            .map(|src| {
                (
                    src.id.to_string(),
                    src.kind,
                    sources::request_url(src.kind, &name, uuid),
                )
            })
            .collect();
        let _ = std::thread::Builder::new()
            .name("supercapes-fetch".into())
            .spawn(move || {
                let msg = match fetch_first(&jobs) {
                    Some((source, bytes, sheet, delay_ms)) => FetchMsg::Pic {
                        uuid,
                        source,
                        bytes,
                        sheet,
                        delay_ms,
                    },
                    None => FetchMsg::Miss { uuid },
                };
                let _ = tx.send(msg);
            });
    }
}

fn fetch_first(
    jobs: &[(String, sources::Kind, String)],
) -> Option<(String, Vec<u8>, bool, Option<u16>)> {
    for (id, kind, url) in jobs {
        if let Some((bytes, sheet, delay)) = fetch_one(*kind, url) {
            return Some((id.clone(), bytes, sheet, delay));
        }
    }
    None
}

fn fetch_one(kind: sources::Kind, url: &str) -> Option<(Vec<u8>, bool, Option<u16>)> {
    let body = http_get(url)?;
    if kind == sources::Kind::MinecraftCapes {
        let text = String::from_utf8_lossy(&body);
        let pic = sources::minecraftcapes_cape_url(&text)?;
        let sheet = sources::minecraftcapes_animated(&text);
        let delay = sources::provider_frame_delay(&text);
        let bytes = http_get(&pic)?;
        return Some((bytes, sheet, delay));
    }
    if body.len() < 32 {
        return None;
    }
    Some((body, false, None))
}

fn http_get(url: &str) -> Option<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .build();
    let resp = agent
        .get(url)
        .set("Accept", "image/png, image/gif, application/json, */*")
        .set("User-Agent", "SuperCapes/1.0.0")
        .call()
        .ok()?;
    if !(200..300).contains(&resp.status()) {
        return None;
    }
    let mut buf = Vec::new();
    resp.into_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.is_empty() {
        return None;
    }
    Some(buf)
}

fn process_image(bytes: &[u8], source: String, sheet: bool, delay_ms: Option<u16>) -> Option<ReadyCape> {
    let pics = if rules::image::is_gif(bytes) {
        rules::image::decode_gif(bytes)?
    } else {
        let (w, h, rgba) = sunrise_client::hud::decode_bytes(bytes).ok()?;
        rules::image::split_sheet(w, h, &rgba, sheet, delay_ms)?
    };
    let hash = sunrise_client::skins::sha1(bytes);
    let mut hex = String::new();
    for b in hash {
        hex.push_str(&format!("{b:02x}"));
    }
    let mut frames = Vec::new();
    for (i, pic) in pics.into_iter().enumerate() {
        if !rules::image::any_visible(&pic.rgba) {
            continue;
        }
        // HD OptiFine capes (92x44, 184x88, ...) back to the 46x22 grid, so the
        // slot keeps the vanilla layout 1:1 instead of stretching to 64x32.
        let (w, h, rgba) = rules::image::fit_cape(pic.width, pic.height, &pic.rgba);
        let slot = sunrise_client::skins::process_cape(w, h, &rgba)?;
        frames.push(CapeFrame {
            location: format!("capes/supercapes-{hex}-f{i}"),
            rgba: slot,
            delay_ms: pic.delay_ms,
        });
    }
    if frames.is_empty() {
        return None;
    }
    if frames.len() == 1 {
        frames[0].location = format!("capes/supercapes-{hex}");
    }
    Some(ReadyCape { source, frames })
}

/// The item worn in the chest slot, as the cape layer reads it.
fn chest_of(m: &sunrise_client::mobs::Mob) -> Option<String> {
    m.extra
        .slot_stack(sunrise_items::equipment::CHEST)
        .map(|s| s.item.to_string())
}

/// Whether `location` is in an atlas slot to draw; if not, uploads it again.
fn keep_slot(game: &mut Game<'_>, location: &str, rgba: &[u8]) {
    if !game.skin_slots.contains(location) || !game.gui.atlas.sprites.contains_key(location) {
        game.store_skin(location.to_string(), rgba.to_vec());
    } else {
        game.skin_downloads.borrow_mut().drawn(location);
    }
}

/// Sets every player's `mob.cape` to the one cape that wins: the first
/// enabled provider's picture (the frame now), else the player's own cape.
/// The vanilla cape layer draws it unless the cloth replaces it (the
/// `cape_layer` mixin skips those players), and the elytra and the screen
/// preview use the same picture (the `cape_layer` and `elytra` mixins draw
/// it again if the game's skin refresh put another one back first). With
/// "Elytra uses cape" off, the `elytra` mixin draws a plain elytra.
///
/// Online off, a source off, or Only me (for other players) drop that
/// provider's picture the same frame, including one already fetched or
/// left on the mob, and put the official cape back. This runs even when
/// Style is Blocky or nothing is in memory, so a stuck fetched cape
/// cannot keep drawing through the vanilla layer.
fn apply_capes(game: &mut Game<'_>) {
    let local = local_player_uuid(game);
    hook::drop_disallowed_capes(local);
    let (elytra_ok, capes, previous_vanilla) = hook::revise(|s| {
        let elapsed = s.anim_origin.elapsed();
        (
            s.prefs.elytra,
            s.capes
                .iter()
                .map(|(u, c)| {
                    let loc = c
                        .location_at(s.prefs.animated, elapsed)
                        .unwrap_or("")
                        .to_string();
                    let frames: Vec<(String, Vec<u8>)> = c
                        .frames
                        .iter()
                        .map(|f| (f.location.clone(), f.rgba.clone()))
                        .collect();
                    (*u, loc, frames)
                })
                .collect::<Vec<_>>(),
            s.vanilla.clone(),
        )
    });
    for (_uuid, _loc, frames) in &capes {
        for (location, rgba) in frames {
            keep_slot(game, location, rgba);
        }
    }
    let ids: Vec<(usize, u32)> = game
        .mobs
        .iter()
        .enumerate()
        .filter(|(_, m)| &*m.kind.id == "minecraft:player")
        .map(|(i, m)| (i, m.id))
        .collect();
    let mut visible: HashMap<u32, String> = HashMap::new();
    let mut vanilla: HashMap<u32, String> = HashMap::new();
    let local_id = game.player_model.as_ref().map(|m| m.id);
    for (i, id) in ids {
        let uuid = game.remote_player_uuid(id);
        let provider = uuid
            .and_then(|u| capes.iter().find(|(x, _, _)| *x == u))
            .map(|(_, l, _)| l.as_str())
            .filter(|l| !l.is_empty());
        let wears_elytra = chest_of(&game.mobs[i]).as_deref() == Some("minecraft:elytra");
        let current = game.mobs[i].cape.clone();
        let (official, loc) = plan_cape(
            current.as_deref(),
            provider,
            previous_vanilla.get(&id).map(String::as_str),
        );
        if let Some(v) = official {
            vanilla.insert(id, v);
        }
        let in_atlas = loc
            .as_ref()
            .is_some_and(|l| game.gui.atlas.sprites.contains_key(l));
        if in_atlas {
            if let Some(l) = loc.clone() {
                visible.insert(id, l);
            }
        }
        write_cape(
            &mut game.mobs[i].cape,
            loc.as_deref(),
            in_atlas,
            !wears_elytra || elytra_ok,
        );
    }
    if let Some(mob_id) = local_id {
        let provider = local
            .and_then(|u| capes.iter().find(|(x, _, _)| *x == u))
            .map(|(_, l, _)| l.as_str())
            .filter(|l| !l.is_empty());
        let wears = game
            .player_model
            .as_ref()
            .map(|m| chest_of(m).as_deref() == Some("minecraft:elytra"))
            .unwrap_or(false);
        let current = game.player_model.as_ref().and_then(|m| m.cape.clone());
        let prev = previous_vanilla
            .get(&u32::MAX)
            .or_else(|| previous_vanilla.get(&mob_id))
            .map(String::as_str);
        let (official, loc) = plan_cape(current.as_deref(), provider, prev);
        if let Some(v) = official.clone() {
            vanilla.insert(u32::MAX, v.clone());
            vanilla.insert(mob_id, v);
        }
        let in_atlas = loc
            .as_ref()
            .is_some_and(|l| game.gui.atlas.sprites.contains_key(l));
        if in_atlas {
            if let Some(l) = loc.clone() {
                visible.insert(u32::MAX, l.clone());
                visible.insert(mob_id, l);
            }
        }
        if let Some(mob) = game.player_model.as_mut() {
            write_cape(&mut mob.cape, loc.as_deref(), in_atlas, !wears || elytra_ok);
        }
    }
    hook::revise(|s| {
        s.visible = visible;
        s.vanilla = vanilla;
        s.local_id = local_id;
    });
}

fn plan_cape(
    current: Option<&str>,
    provider: Option<&str>,
    prev_official: Option<&str>,
) -> (Option<String>, Option<String>) {
    let official = sources::official_cape(current, prev_official);
    let loc = sources::pick_cape(provider, current, official.as_deref());
    (official, loc)
}

/// Write the chosen picture onto the mob, or clear a fetched one we must
/// no longer show. `set_on_mob` is false while an elytra is worn with
/// "Elytra uses cape" off, matching the earlier behaviour.
fn write_cape(cape: &mut Option<String>, loc: Option<&str>, loc_in_atlas: bool, set_on_mob: bool) {
    let fetched_on_mob = cape.as_deref().is_some_and(sources::is_fetched_sprite);
    if set_on_mob {
        match loc {
            Some(l) if loc_in_atlas || !sources::is_fetched_sprite(l) => {
                *cape = Some(l.to_string());
            }
            None if fetched_on_mob => {
                *cape = None;
            }
            _ => {}
        }
    } else if fetched_on_mob && loc != cape.as_deref() {
        // Strip a fetched picture we must no longer show, so it cannot
        // leak onto the wings if the elytra mixin misses.
        *cape = None;
    }
}

/// Whether the vanilla cape layer should skip `m` this frame because the
/// cloth draws its cape (the `cape_layer` mixin).
pub fn cloth_replaces_vanilla(game: &Game<'_>, m: &sunrise_client::mobs::Mob) -> bool {
    let local = game.player_model.as_ref().is_some_and(|p| p.id == m.id);
    hook::cloth_replaces_vanilla(m.id, local)
}

/// The winning cape picture for `m` when the game is about to draw a
/// different one (the game's skin refresh can put the player's own cape
/// back between our frame update and the draw): the vanilla cape layer and
/// the elytra are then drawn again with ours. `None` keeps the game's.
pub fn cape_override(game: &Game<'_>, m: &sunrise_client::mobs::Mob, elytra: bool) -> Option<String> {
    let local = game.player_model.as_ref().is_some_and(|p| p.id == m.id);
    let sprite = hook::winning_sprite(m.id, local)?;
    if m.cape.as_deref() == Some(sprite.as_str()) || !game.gui.atlas.sprites.contains_key(&sprite) {
        return None;
    }
    Some(sprite)
}

/// What the wings should use as `m`'s cape: `Some(Some(picture))` the
/// winning cape picture ("Elytra uses cape" on), `Some(None)` no cape at
/// all, so the plain elytra texture ("Elytra uses cape" off, never the
/// game's own cape), or `None` to draw as the game would (already right).
pub fn elytra_cape(game: &Game<'_>, m: &sunrise_client::mobs::Mob) -> Option<Option<String>> {
    let (ready, on) = hook::revise(|s| (s.ready, s.prefs.elytra));
    if !ready {
        return None;
    }
    if !on {
        return m.cape.is_some().then_some(None);
    }
    cape_override(game, m, true).map(Some)
}

/// What the cloth needs of each player that the typed draw hook can't read.
fn note_bodies(game: &Game<'_>) {
    if !hook::listen_render() {
        return;
    }
    let mut bodies = HashMap::new();
    let body = |m: &sunrise_client::mobs::Mob| {
        use sunrise_client::pose::Pose;
        hook::Body {
            previous: [m.previous.x, m.previous.y, m.previous.z],
            position: [m.position.x, m.position.y, m.position.z],
            crouching: m.extra.crouching,
            fall_flying: m.extra.fall_flying,
            chest: chest_of(m),
            sleeping: m.extra.avatar_pose == Pose::Sleeping,
            // Crawling uses the swimming pose. Riptide (spin attack) and the
            // swim-amount tip also lay the body over, same as a swimmer.
            swimming: matches!(m.extra.avatar_pose, Pose::Swimming | Pose::SpinAttack)
                || m.extra.swim.visually
                || m.extra.swim.old > 0.0
                || m.extra.swim.amount > 0.0,
        }
    };
    for m in game.mobs.iter().filter(|m| &*m.kind.id == "minecraft:player") {
        bodies.insert(m.id, body(m));
    }
    if let Some(m) = game.player_model.as_ref() {
        let mut b = body(m);
        // The local player's worn chest item, as the inventory has it.
        if let Some(stack) = game.armor[1].as_ref() {
            b.chest = Some(stack.item.to_string());
        }
        bodies.insert(u32::MAX, b.clone());
        bodies.insert(m.id, b);
    }
    hook::revise(|s| s.bodies = bodies);
}
