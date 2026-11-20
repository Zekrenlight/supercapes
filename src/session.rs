// SPDX-License-Identifier: MIT
// SuperCapes session state, shared by the typed hooks (panel, commands, cloth)
// and the game-side support module (cape fetches, atlas slots, mob capes).

use crate::logic as rules;
use rules::{physics, Prefs};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const MOD_ID: &str = "supercapes";
pub const TITLE: &str = "SuperCapes";
pub const VERSION: &str = "1.0.0";
pub const BLURB: &str =
    "Cape cloth that moves with you, and public cape pictures, for Sunrise. No extra keybind.";
pub const AUTHOR: &str = "Zekrenlight";

/// The chest item that hides the cape (vanilla draws the cape on the wings).
pub const ELYTRA: &str = "minecraft:elytra";

/// Set by the game side while the inventory/screen preview draws the
/// player: the preview keeps the plain cape (no cloth there).
static PREVIEW: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct CapeFrame {
    pub location: String,
    pub rgba: Vec<u8>,
    pub delay_ms: u16,
}

#[derive(Clone, Debug)]
pub struct ReadyCape {
    pub source: String,
    pub frames: Vec<CapeFrame>,
}

impl ReadyCape {
    pub fn location_at(&self, animated: bool, elapsed: Duration) -> Option<&str> {
        let delays: Vec<u16> = self.frames.iter().map(|f| f.delay_ms).collect();
        let i = rules::image::frame_index(&delays, elapsed.as_millis() as u64, animated);
        self.frames.get(i).map(|f| f.location.as_str())
    }
}

/// A finished download, sent by a fetch worker to the game side.
pub enum FetchMsg {
    Pic {
        uuid: u128,
        source: String,
        bytes: Vec<u8>,
        sheet: bool,
        delay_ms: Option<u16>,
    },
    Miss {
        uuid: u128,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Motion,
    Sources,
    About,
}

impl Tab {
    pub fn from_index(i: usize) -> Self {
        match i {
            1 => Self::Sources,
            2 => Self::About,
            _ => Self::Motion,
        }
    }
    pub fn index(self) -> usize {
        match self {
            Self::Motion => 0,
            Self::Sources => 1,
            Self::About => 2,
        }
    }
}

/// What the game side saw of a player this frame, for the cloth drawn in
/// `render_player_post` (the typed hook has no crouch, glide, sleep or armour
/// reads for other players).
#[derive(Clone, Debug, Default)]
pub struct Body {
    pub previous: [f64; 3],
    pub position: [f64; 3],
    pub crouching: bool,
    pub fall_flying: bool,
    pub chest: Option<String>,
    /// `Pose.SLEEPING`: the body is laid in the bed.
    pub sleeping: bool,
    /// Swimming, crawling, or riptide (`Pose.SWIMMING` / `SPIN_ATTACK`, or
    /// the swim-amount tip): the body is already laid over.
    pub swimming: bool,
}

impl Body {
    /// The cloth is drawn in a standing frame (at the feet, turned with body
    /// yaw only). Vanilla's cape layer uses the full body transform, which
    /// lays a sleeper, swimmer or crawler over; the cloth would hang through
    /// the head in those poses, so it yields to vanilla.
    pub fn laid_over(&self) -> bool {
        self.sleeping || self.swimming
    }
}

pub struct Session {
    pub prefs: Prefs,
    /// Prefs were read from disk (by the game side, on its first frame).
    pub ready: bool,
    /// Prefs changed in the panel: the game side writes them on its next frame.
    pub prefs_dirty: bool,
    pub overlay: bool,
    pub return_to_pause: bool,
    pub want_close: bool,
    /// The panel asked for the mod screen; the next `render_frame_pre` opens it.
    pub want_open: bool,
    pub tab: Tab,
    pub scroll: f32,
    pub hover: Option<String>,
    pub status: String,
    pub cloth: HashMap<u32, physics::Cloth>,
    pub capes: HashMap<u128, ReadyCape>,
    /// Mob id → cape sprite drawn this frame (`u32::MAX` is the local player).
    pub visible: HashMap<u32, String>,
    /// Mob id → the last official (non-fetched) cape seen on that player,
    /// so turning a source off can put it back the same frame.
    pub vanilla: HashMap<u32, String>,
    pub bodies: HashMap<u32, Body>,
    pub local_id: Option<u32>,
    pub anim_origin: Instant,
    pub in_flight: HashMap<u128, Instant>,
    pub tx: Option<Sender<FetchMsg>>,
    pub rx: Option<Receiver<FetchMsg>>,
}

impl Default for Session {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            prefs: Prefs::default(),
            ready: false,
            prefs_dirty: false,
            overlay: false,
            return_to_pause: false,
            want_close: false,
            want_open: false,
            tab: Tab::Motion,
            scroll: 0.0,
            hover: None,
            status: String::new(),
            cloth: HashMap::new(),
            capes: HashMap::new(),
            visible: HashMap::new(),
            vanilla: HashMap::new(),
            bodies: HashMap::new(),
            local_id: None,
            anim_origin: Instant::now(),
            in_flight: HashMap::new(),
            tx: Some(tx),
            rx: Some(rx),
        }
    }
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Runs `f` on the session. Not re-entrant: never call it from inside `f`.
pub fn revise<R>(f: impl FnOnce(&mut Session) -> R) -> R {
    let mut guard = SESSION.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    f(guard.get_or_insert_with(Session::default))
}

pub fn request_close() {
    revise(|s| s.want_close = true);
}

pub fn open_overlay() {
    revise(|s| {
        s.overlay = true;
        s.return_to_pause = true;
        s.want_close = false;
    });
}

pub fn mark_prefs_dirty() {
    revise(|s| s.prefs_dirty = true);
}

pub fn prefs_snapshot() -> Prefs {
    revise(|s| s.prefs.clone())
}

/// Drops in-memory fetched capes that the current prefs would not show,
/// so apply and a later fetch see the same set. The on-disk cache is left
/// alone so turning a source back on can reuse it. When Only me is on and
/// the local UUID is not known yet, existing capes are kept in memory;
/// apply still hides them on everyone but the local player.
pub fn drop_disallowed_capes(local_uuid: Option<u128>) {
    revise(|s| drop_disallowed_capes_in(s, local_uuid));
}

pub fn drop_disallowed_capes_in(s: &mut Session, local_uuid: Option<u128>) {
    s.capes.retain(|uuid, cape| {
        if !s.prefs.online {
            return false;
        }
        let is_local = match local_uuid {
            Some(local) => *uuid == local,
            None if s.prefs.only_self => return true,
            None => false,
        };
        s.prefs.provider_allowed(&cape.source, is_local)
    });
}

pub fn listen_render() -> bool {
    revise(|s| s.ready && s.prefs.cloth())
}

pub fn cape_sprite(id: u32) -> Option<String> {
    revise(|s| s.visible.get(&id).cloned())
}

pub fn set_preview(on: bool) {
    PREVIEW.store(on, Ordering::Relaxed);
}

fn pick<V>(m: &HashMap<u32, V>, id: u32, local: bool) -> Option<u32> {
    if m.contains_key(&id) {
        Some(id)
    } else if local && m.contains_key(&u32::MAX) {
        Some(u32::MAX)
    } else {
        None
    }
}

/// The cape picture that wins for a player this frame (first enabled
/// provider, else the player's own), once the prefs are read.
pub fn winning_sprite(id: u32, local: bool) -> Option<String> {
    revise(|s| {
        if !s.ready {
            return None;
        }
        let key = pick(&s.visible, id, local)?;
        s.visible.get(&key).cloned()
    })
}

/// The cape sprite and body for a player SuperCapes draws as cloth this
/// frame, or `None` when the vanilla cape layer should draw instead (cloth
/// off, no cape, an elytra worn: vanilla puts the cape on the wings, or a
/// laid-over pose: sleep, swim, crawl or riptide, where the cloth's
/// standing frame would hang through the head).
/// `local` also looks under the local player's key (`u32::MAX`).
pub fn cloth_target(id: u32, local: bool) -> Option<(String, Option<Body>)> {
    revise(|s| {
        if !s.ready || !s.prefs.cloth() {
            return None;
        }
        let key = pick(&s.visible, id, local)?;
        let sprite = s.visible.get(&key)?.clone();
        let body = pick(&s.bodies, id, local).and_then(|k| s.bodies.get(&k)).cloned();
        if body.as_ref().is_some_and(|b| b.chest.as_deref() == Some(ELYTRA) || b.laid_over()) {
            return None;
        }
        Some((sprite, body))
    })
}

/// Whether the vanilla cape layer must skip this player because the cloth
/// draws its cape (one cape per player). The screen preview keeps vanilla's.
pub fn cloth_replaces_vanilla(id: u32, local: bool) -> bool {
    !PREVIEW.load(Ordering::Relaxed) && cloth_target(id, local).is_some()
}

pub fn cloth_for(id: u32, prefs: &Prefs) -> physics::Cloth {
    revise(|s| {
        let cp = physics::ClothPrefs::from_prefs(prefs);
        s.cloth
            .entry(id)
            .and_modify(|c| c.resize(cp.quality, cp.style))
            .or_insert_with(|| physics::Cloth::new(cp.quality, cp.style))
            .clone()
    })
}

pub fn store_cloth(id: u32, cloth: physics::Cloth) {
    revise(|s| {
        s.cloth.insert(id, cloth);
    });
}
