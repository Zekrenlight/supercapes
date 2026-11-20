// SPDX-License-Identifier: MIT
// SuperCapes rules: cloth motion, cape sources, prefs, layout, and close.
// Pure rules (no game runtime); tested by `cargo test --lib` (src/rules_tests.rs).

pub use close::{apply_take_over, auto_return_to_pause, decide_close, should_drop_claim, Destination};
pub use persist::{dump_prefs, load_prefs, Prefs, FILE_NAME, FORMAT};

pub mod persist {
    use super::{physics, sources};

    pub const FILE_NAME: &str = "supercapes.cfg";
    /// 3: Style is the one cape control (Smooth cloth or Blocky vanilla);
    /// the `cloth` and `motion` keys of format 2 become a Style. Format 1
    /// (the pre-release) also gets the calm motion defaults.
    pub const FORMAT: u32 = 3;

    #[derive(Clone, Debug, PartialEq)]
    pub struct Prefs {
        pub format: u32,
        pub online: bool,
        pub only_self: bool,
        pub elytra: bool,
        pub animated: bool,
        /// Smooth: the segmented cloth. Blocky: the rigid vanilla cape.
        pub style: physics::Style,
        pub wind: physics::Wind,
        pub quality: physics::Quality,
        pub gravity: u8,
        pub wind_strength: u8,
        pub sources: Vec<(String, bool)>,
    }

    impl Prefs {
        /// Whether SuperCapes draws the cape as cloth (Smooth); Blocky
        /// leaves the vanilla cape layer to draw it.
        pub fn cloth(&self) -> bool {
            self.style == physics::Style::Smooth
        }

        /// Whether a picture from this online source may be shown on this
        /// player. Online off, the source off, or Only me (for anyone but
        /// the local player) all hide it, including a cape already fetched.
        pub fn provider_allowed(&self, source: &str, is_local: bool) -> bool {
            self.online
                && !(self.only_self && !is_local)
                && self.sources.iter().any(|(id, on)| *on && id == source)
        }
    }

    impl Default for Prefs {
        fn default() -> Self {
            Self {
                format: FORMAT,
                online: true,
                only_self: false,
                elytra: true,
                animated: true,
                style: physics::Style::Smooth,
                wind: physics::Wind::Waves,
                quality: physics::Quality::Medium,
                gravity: 5,
                wind_strength: 4,
                sources: sources::default_toggles(),
            }
        }
    }

    fn flag(v: bool) -> &'static str {
        if v {
            "yes"
        } else {
            "no"
        }
    }

    fn parse_flag(v: &str) -> bool {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "yes" | "true" | "on" | "1"
        )
    }

    pub fn dump_prefs(p: &Prefs) -> String {
        let mut s = String::new();
        s.push_str(&format!("format={}\n", p.format.max(FORMAT)));
        s.push_str(&format!("online={}\n", flag(p.online)));
        s.push_str(&format!("only_self={}\n", flag(p.only_self)));
        s.push_str(&format!("elytra={}\n", flag(p.elytra)));
        s.push_str(&format!("animated={}\n", flag(p.animated)));
        s.push_str(&format!("style={}\n", p.style.name()));
        s.push_str(&format!("wind={}\n", p.wind.name()));
        s.push_str(&format!("quality={}\n", p.quality.name()));
        s.push_str(&format!("gravity={}\n", p.gravity.min(9)));
        s.push_str(&format!("wind_strength={}\n", p.wind_strength.min(9)));
        let order: Vec<String> = p.sources.iter().map(|(id, _)| id.clone()).collect();
        s.push_str(&format!("order={}\n", order.join(",")));
        for (id, on) in &p.sources {
            s.push_str(&format!("source.{id}={}\n", flag(*on)));
        }
        s
    }

    pub fn load_prefs(text: &str) -> Prefs {
        let mut p = Prefs::default();
        let mut on: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
        let mut order: Option<Vec<String>> = None;
        // Format 2 and older: the cloth toggle and the motion setting.
        let mut old_cloth = true;
        let mut old_follow = false;
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let k = k.trim();
            let v = v.trim();
            match k {
                "format" => {
                    if let Ok(n) = v.parse() {
                        p.format = n;
                    }
                }
                "online" => p.online = parse_flag(v),
                "only_self" => p.only_self = parse_flag(v),
                "elytra" => p.elytra = parse_flag(v),
                "animated" => p.animated = parse_flag(v),
                "cloth" => old_cloth = parse_flag(v),
                "style" => p.style = physics::Style::from_name(v),
                "wind" => p.wind = physics::Wind::from_name(v),
                "motion" => old_follow = v.eq_ignore_ascii_case("follow"),
                "quality" => p.quality = physics::Quality::from_name(v),
                "gravity" => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.gravity = n.min(9);
                    }
                }
                "wind_strength" => {
                    if let Ok(n) = v.parse::<u8>() {
                        p.wind_strength = n.min(9);
                    }
                }
                "order" => {
                    order = Some(
                        v.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    );
                }
                k if k.starts_with("source.") => {
                    on.insert(k[7..].to_string(), parse_flag(v));
                }
                _ => {}
            }
        }
        p.sources = sources::merge_toggles(order.as_deref(), &on);
        if p.format < 2 {
            // The pre-release's wind settings: back to the calm defaults
            // (its motion setting is dropped with them).
            let d = Prefs::default();
            p.wind = d.wind;
            p.quality = d.quality;
            p.gravity = d.gravity;
            p.wind_strength = d.wind_strength;
            old_follow = false;
        }
        if p.format < 3 {
            // Style did nothing before; cloth off or Follow looked Blocky.
            p.style = if !old_cloth || old_follow {
                physics::Style::Blocky
            } else {
                physics::Style::Smooth
            };
            p.format = FORMAT;
        }
        p
    }
}

pub mod sources {
    /// A well-known public cape source. SuperCapes only requests a public
    /// cape picture for a username or UUID; it sends nothing else.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Source {
        pub id: &'static str,
        pub title: &'static str,
        pub default_on: bool,
        pub kind: Kind,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Kind {
        /// `http://s.optifine.net/capes/{name}.png`
        OptiFine,
        /// `https://api.minecraftcapes.net/profile/{uuid}` then a cape URL.
        MinecraftCapes,
        /// `https://items.laby.net/<cloak-item>/textures/{uuid}.png`
        LabyMod,
        /// `https://api.cloaks.gg/users/{uuid}/cape`
        Cloaks,
    }

    /// LabyMod's public cloak-item folder id, as used by their texture host.
    pub const LABY_CLOAK_ITEM: &str = "00e8db8c-79b2-4158-88a7-7c2aa17121b3";

    pub const ALL: &[Source] = &[
        Source {
            id: "optifine",
            title: "OptiFine",
            default_on: true,
            kind: Kind::OptiFine,
        },
        Source {
            id: "minecraftcapes",
            title: "MinecraftCapes",
            default_on: true,
            kind: Kind::MinecraftCapes,
        },
        Source {
            id: "labymod",
            title: "LabyMod",
            default_on: false,
            kind: Kind::LabyMod,
        },
        Source {
            id: "cloaks",
            title: "Cloaks",
            default_on: false,
            kind: Kind::Cloaks,
        },
    ];

    pub fn by_id(id: &str) -> Option<&'static Source> {
        ALL.iter().find(|s| s.id == id)
    }

    pub fn default_toggles() -> Vec<(String, bool)> {
        ALL.iter().map(|s| (s.id.to_string(), s.default_on)).collect()
    }

    pub fn merge_toggles(
        order: Option<&[String]>,
        on: &std::collections::BTreeMap<String, bool>,
    ) -> Vec<(String, bool)> {
        let mut out: Vec<(String, bool)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let push = |out: &mut Vec<(String, bool)>, seen: &mut std::collections::BTreeSet<String>, id: &str| {
            if !ALL.iter().any(|s| s.id == id) || !seen.insert(id.to_string()) {
                return;
            }
            let default = ALL.iter().find(|s| s.id == id).map(|s| s.default_on).unwrap_or(false);
            out.push((id.to_string(), on.get(id).copied().unwrap_or(default)));
        };
        if let Some(order) = order {
            for id in order {
                push(&mut out, &mut seen, id);
            }
        }
        for s in ALL {
            push(&mut out, &mut seen, s.id);
        }
        out
    }

    pub fn enabled_in_order(prefs: &[(String, bool)]) -> Vec<&'static Source> {
        prefs
            .iter()
            .filter(|(_, on)| *on)
            .filter_map(|(id, _)| by_id(id))
            .collect()
    }

    pub fn move_source(prefs: &mut Vec<(String, bool)>, id: &str, up: bool) -> bool {
        let Some(i) = prefs.iter().position(|(s, _)| s == id) else {
            return false;
        };
        if up {
            if i == 0 {
                return false;
            }
            prefs.swap(i, i - 1);
        } else {
            if i + 1 >= prefs.len() {
                return false;
            }
            prefs.swap(i, i + 1);
        }
        true
    }

    /// Sprite locations SuperCapes uploads (`capes/supercapes-{hash}` and
    /// `capes/supercapes-{hash}-fN`). Used to tell a fetched picture from
    /// the player's official cape, so turning a source off can put the
    /// official one back the same frame.
    pub const FETCHED_PREFIX: &str = "capes/supercapes-";

    pub fn is_fetched_sprite(location: &str) -> bool {
        location.starts_with(FETCHED_PREFIX)
    }

    /// The official (non-fetched) cape to remember or restore. A picture
    /// still on the mob wins if it is not one we uploaded; otherwise the
    /// last official one we saw is kept. A fetched sprite is never kept.
    pub fn official_cape(current: Option<&str>, remembered: Option<&str>) -> Option<String> {
        for loc in [current, remembered] {
            if let Some(c) = loc.filter(|s| !s.is_empty() && !is_fetched_sprite(s)) {
                return Some(c.to_string());
            }
        }
        None
    }

    /// The cape picture that should be on the player this frame. An allowed
    /// provider picture wins; otherwise the official cape is restored. A
    /// fetched sprite left on the mob is never kept once the provider is
    /// disallowed, so Online / source toggles take effect immediately.
    pub fn pick_cape(
        provider: Option<&str>,
        current: Option<&str>,
        remembered: Option<&str>,
    ) -> Option<String> {
        if let Some(p) = provider.filter(|s| !s.is_empty()) {
            return Some(p.to_string());
        }
        official_cape(current, remembered)
    }

    fn hyphenated_uuid(uuid: u128) -> String {
        let h = format!("{uuid:032x}");
        format!(
            "{}-{}-{}-{}-{}",
            &h[0..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..32]
        )
    }

    fn compact_uuid(uuid: u128) -> String {
        format!("{uuid:032x}")
    }

    /// The public URL this source is asked, or a JSON profile URL that names
    /// the picture. Only the player's name or UUID is placed in the path.
    pub fn request_url(kind: Kind, name: &str, uuid: u128) -> String {
        match kind {
            Kind::OptiFine => format!("http://s.optifine.net/capes/{name}.png"),
            Kind::MinecraftCapes => {
                format!("https://api.minecraftcapes.net/profile/{}", compact_uuid(uuid))
            }
            Kind::LabyMod => format!(
                "https://items.laby.net/{LABY_CLOAK_ITEM}/textures/{}.png",
                hyphenated_uuid(uuid)
            ),
            Kind::Cloaks => format!(
                "https://api.cloaks.gg/users/{}/cape",
                hyphenated_uuid(uuid)
            ),
        }
    }

    /// MinecraftCapes answers JSON; the cape picture is at `cape_url`.
    pub fn minecraftcapes_cape_url(json: &str) -> Option<String> {
        // Tiny parser: look for "cape_url":"..." without pulling in serde here.
        let key = "\"cape_url\"";
        let at = json.find(key)?;
        let rest = &json[at + key.len()..];
        let rest = rest.trim_start();
        let rest = rest.strip_prefix(':')?.trim_start();
        if rest.starts_with("null") {
            return None;
        }
        let rest = rest.strip_prefix('"')?;
        let end = rest.find('"')?;
        let url = rest[..end].trim();
        if url.starts_with("http://") || url.starts_with("https://") {
            Some(url.to_string())
        } else {
            None
        }
    }

    pub fn minecraftcapes_animated(json: &str) -> bool {
        if let Some(at) = json.find("\"animatedCape\"") {
            let rest = json[at..].to_ascii_lowercase();
            return rest.contains("true");
        }
        false
    }

    /// Optional frame time from a profile JSON, in milliseconds.
    pub fn provider_frame_delay(json: &str) -> Option<u16> {
        const KEYS: &[&str] = &[
            "\"frameDelay\"",
            "\"frame_delay\"",
            "\"animationDelay\"",
            "\"animation_delay\"",
            "\"capeFrameDelay\"",
        ];
        for key in KEYS {
            if let Some(n) = json_u32_after(json, key) {
                let ms = if n > 0 && n <= 20 { n * 10 } else { n };
                return Some((ms as u16).clamp(20, 60_000));
            }
        }
        None
    }

    fn json_u32_after(json: &str, key: &str) -> Option<u32> {
        let at = json.find(key)?;
        let rest = json[at + key.len()..].trim_start();
        let rest = rest.strip_prefix(':')?.trim_start();
        let rest = rest.trim_start_matches('"');
        let num: String = rest
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if num.is_empty() {
            return None;
        }
        num.parse().ok()
    }
}

pub mod physics {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Style {
        Blocky,
        Smooth,
    }

    impl Style {
        pub fn name(self) -> &'static str {
            match self {
                Self::Blocky => "blocky",
                Self::Smooth => "smooth",
            }
        }
        pub fn label(self) -> &'static str {
            match self {
                Self::Blocky => "Blocky",
                Self::Smooth => "Smooth",
            }
        }
        pub fn from_name(s: &str) -> Self {
            if s.eq_ignore_ascii_case("blocky") {
                Self::Blocky
            } else {
                Self::Smooth
            }
        }
        pub fn next(self) -> Self {
            match self {
                Self::Blocky => Self::Smooth,
                Self::Smooth => Self::Blocky,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Wind {
        Off,
        Waves,
    }

    impl Wind {
        pub fn name(self) -> &'static str {
            match self {
                Self::Off => "off",
                Self::Waves => "waves",
            }
        }
        pub fn label(self) -> &'static str {
            match self {
                Self::Off => "Ripple: Off",
                Self::Waves => "Ripple: When moving",
            }
        }
        pub fn from_name(s: &str) -> Self {
            if s.eq_ignore_ascii_case("waves") {
                Self::Waves
            } else {
                Self::Off
            }
        }
        pub fn next(self) -> Self {
            match self {
                Self::Off => Self::Waves,
                Self::Waves => Self::Off,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Motion {
        Follow,
        Cloth,
        Cloth3d,
    }

    impl Motion {
        pub fn name(self) -> &'static str {
            match self {
                Self::Follow => "follow",
                Self::Cloth => "cloth",
                Self::Cloth3d => "cloth3d",
            }
        }
        pub fn label(self) -> &'static str {
            match self {
                Self::Follow => "Follow",
                Self::Cloth => "Cloth",
                Self::Cloth3d => "Cloth 3D",
            }
        }
        pub fn from_name(s: &str) -> Self {
            match s.to_ascii_lowercase().as_str() {
                "follow" => Self::Follow,
                "cloth" => Self::Cloth,
                _ => Self::Cloth3d,
            }
        }
        pub fn next(self) -> Self {
            match self {
                Self::Follow => Self::Cloth,
                Self::Cloth => Self::Cloth3d,
                Self::Cloth3d => Self::Follow,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Quality {
        Low,
        Medium,
        High,
    }

    impl Quality {
        pub fn name(self) -> &'static str {
            match self {
                Self::Low => "low",
                Self::Medium => "medium",
                Self::High => "high",
            }
        }
        pub fn label(self) -> &'static str {
            match self {
                Self::Low => "Low",
                Self::Medium => "Medium",
                Self::High => "High",
            }
        }
        pub fn from_name(s: &str) -> Self {
            match s.to_ascii_lowercase().as_str() {
                "low" => Self::Low,
                "high" => Self::High,
                _ => Self::Medium,
            }
        }
        pub fn next(self) -> Self {
            match self {
                Self::Low => Self::Medium,
                Self::Medium => Self::High,
                Self::High => Self::Low,
            }
        }
        /// Segments down the (Smooth) cape: Low 8, Medium 16, High 24.
        pub fn segments(self, _style: Style) -> usize {
            match self {
                Self::Low => 8,
                Self::Medium => 16,
                Self::High => 24,
            }
        }
    }

    /// The vanilla cape box: 10 x 16 x 1 model pixels at the player
    /// renderer's 15/16 scale, in blocks. The cloth is exactly this box.
    pub const MODEL_SCALE: f32 = 0.9375;
    pub const CAPE_W: f32 = 10.0 / 16.0 * MODEL_SCALE;
    pub const CAPE_H: f32 = 16.0 / 16.0 * MODEL_SCALE;
    pub const CAPE_T: f32 = 1.0 / 16.0 * MODEL_SCALE;
    /// Largest outward swing of the top segment (degrees from hanging
    /// straight down); lower segments may bend up to `MAX_BEND` more.
    pub const MAX_SWING: f32 = 70.0;
    pub const MAX_BEND: f32 = 25.0;
    /// Largest sideways sway, in degrees.
    pub const MAX_ROLL: f32 = 12.0;
    /// Extra outward tilt while crouching, so the cape clears the bent body.
    pub const SNEAK_TILT: f32 = 25.0;
    /// Largest travelling-wave amplitude at full speed with Ripple at 9.
    pub const MAX_RIPPLE: f32 = 18.0;
    /// How fast the top segment follows the motion, and each segment the
    /// one above it (per tick): the lag down the chain makes the delay.
    const TOP_RATE: f32 = 1.0;
    const CHAIN_RATE: f32 = 2.6;
    /// Extra bend toward the hem, as a share of the top's swing.
    const BEND: f32 = 0.35;

    /// A segmented cape (WaveyCapes-style). Each segment follows the one
    /// above it with a lag, so a change at the collar travels down as a
    /// wave and the hem bends further. What drives the collar is the
    /// vanilla cape's motion model: a cloak point trails the player (a
    /// quarter of the gap each tick); moving forward or falling lifts the
    /// cape, strafing and turning swing it sideways, crouching tilts it.
    /// While moving, a travelling ripple runs down it. There is no wind or
    /// idle noise: standing still, everything settles to 0 and the cape
    /// hangs straight down.
    #[derive(Clone, Debug)]
    pub struct Cloth {
        /// Segment count (rows of vertices = segments + 1).
        pub segments: usize,
        /// Outward angle of each segment, degrees (0 = straight down).
        angle: Vec<f32>,
        /// Sideways sway of each segment, degrees.
        roll: Vec<f32>,
        cloak: [f32; 3],
        have: bool,
        last_age: f32,
        last_yaw: f32,
        /// Smoothed collar swing target, sway target and ripple size.
        swing: f32,
        sway: f32,
        ripple: f32,
        phase: f32,
        motion: Motion,
    }

    fn wrap_deg(a: f32) -> f32 {
        let mut a = a % 360.0;
        if a > 180.0 {
            a -= 360.0;
        }
        if a < -180.0 {
            a += 360.0;
        }
        a
    }

    fn snap(v: &mut f32) {
        if v.abs() < 1e-3 || !v.is_finite() {
            *v = 0.0;
        }
    }

    /// One face of the cape box: four corners in the player's frame (in the
    /// order the draw call wants: top left, bottom left, bottom right, top
    /// right seen from outside) and its uv rectangle in 16ths of the 64x32
    /// cape picture.
    pub type Face = ([[f32; 3]; 4], [f32; 4]);

    /// Texels of the 64x32 cape picture as the draw call's 16-unit uv.
    pub fn uv_px(u0: f32, v0: f32, u1: f32, v1: f32) -> [f32; 4] {
        [u0 / 64.0 * 16.0, v0 / 32.0 * 16.0, u1 / 64.0 * 16.0, v1 / 32.0 * 16.0]
    }

    impl Cloth {
        pub fn new(quality: Quality, style: Style) -> Self {
            let n = quality.segments(style).max(1);
            Self {
                segments: n,
                angle: vec![0.0; n],
                roll: vec![0.0; n],
                cloak: [0.0; 3],
                have: false,
                last_age: 0.0,
                last_yaw: 0.0,
                swing: 0.0,
                sway: 0.0,
                ripple: 0.0,
                phase: 0.0,
                motion: Motion::Cloth,
            }
        }

        pub fn resize(&mut self, quality: Quality, style: Style) {
            let n = quality.segments(style).max(1);
            if n != self.segments {
                // Resample the chain so a settings change doesn't jolt it.
                let pick = |v: &Vec<f32>| -> Vec<f32> {
                    (0..n).map(|i| v[i * v.len() / n]).collect()
                };
                self.angle = pick(&self.angle);
                self.roll = pick(&self.roll);
                self.segments = n;
            }
        }

        /// (collar swing, sway, ripple) in degrees. All settle to 0 at rest.
        pub fn angles(&self) -> (f32, f32, f32) {
            (self.swing, self.sway, self.ripple)
        }

        /// Each segment's drawn angle (degrees, wave included) and sway.
        pub fn segment_angles(&self) -> Vec<(f32, f32)> {
            let n = self.segments;
            (0..n)
                .map(|i| {
                    let t = (i as f32 + 0.5) / n as f32;
                    let wave = self.ripple
                        * t.powf(0.8)
                        * (self.phase - t * std::f32::consts::TAU * 0.9).sin();
                    let a = (self.angle[i] + wave).clamp(0.0, MAX_SWING + MAX_BEND);
                    (a, self.roll[i])
                })
                .collect()
        }

        /// Advance to the player's state at `age` ticks (partial ticks
        /// allowed). `world` is the interpolated player position, `body_yaw`
        /// the body yaw in degrees (vanilla convention).
        pub fn step(
            &mut self,
            world: [f32; 3],
            body_yaw: f32,
            age: f32,
            sneaking: bool,
            gliding: bool,
            prefs: ClothPrefs,
        ) {
            self.motion = prefs.motion;
            let far = (0..3).any(|i| (world[i] - self.cloak[i]).abs() > 10.0);
            if !self.have || far || !age.is_finite() {
                self.cloak = world;
                self.last_age = age;
                self.last_yaw = body_yaw;
                self.have = true;
            }
            let dt = (age - self.last_age).clamp(0.0, 4.0);
            self.last_age = age;
            // Vanilla: the cloak closes a quarter of the gap each tick.
            let k = 1.0 - 0.75f32.powf(dt);
            for i in 0..3 {
                self.cloak[i] += (world[i] - self.cloak[i]) * k;
            }
            let d = [
                self.cloak[0] - world[0],
                self.cloak[1] - world[1],
                self.cloak[2] - world[2],
            ];
            let yaw = body_yaw.to_radians();
            let (s, c) = (yaw.sin(), -yaw.cos());
            let forward = d[0] * s + d[2] * c;
            let sideways = d[0] * c - d[2] * s;
            // Weight 0..9: 0 lifts like the vanilla cape, heavier lifts less.
            let weight = 1.0 - f32::from(prefs.gravity.min(9)) * 0.07;
            let glide = if gliding { 0.0 } else { 1.0 };
            let lean = (forward * 100.0 * glide).clamp(0.0, 150.0) * 0.5 * weight;
            let flap = (d[1] * 10.0).clamp(-6.0, 32.0) * weight;
            let mut target_swing = (lean + flap).clamp(0.0, MAX_SWING);
            if sneaking {
                target_swing = (target_swing + SNEAK_TILT).min(MAX_SWING);
            }
            let turn = if dt > 1e-4 {
                wrap_deg(body_yaw - self.last_yaw) / dt
            } else {
                0.0
            };
            self.last_yaw = body_yaw;
            let side = (sideways * 100.0).clamp(-20.0, 20.0) * 0.5;
            let turn_sway = match prefs.motion {
                Motion::Follow => 0.0,
                Motion::Cloth => -turn * 0.5,
                Motion::Cloth3d => -turn * 0.8,
            };
            let target_sway = (side + turn_sway).clamp(-MAX_ROLL, MAX_ROLL);
            let speed = (d[0] * d[0] + d[2] * d[2]).sqrt();
            let moving = (speed / 0.8).min(1.0);
            let target_ripple = match (prefs.wind, prefs.motion) {
                (Wind::Waves, Motion::Cloth | Motion::Cloth3d) => {
                    MAX_RIPPLE * f32::from(prefs.wind_strength.min(9)) / 9.0 * moving
                }
                _ => 0.0,
            };
            // Substeps keep the chain stable at low frame rates.
            let steps = (dt / 0.25).ceil().max(1.0) as usize;
            let h = dt / steps as f32;
            let top = 1.0 - (-TOP_RATE * h).exp();
            let link = 1.0 - (-CHAIN_RATE * h).exp();
            let n = self.segments;
            for _ in 0..steps {
                self.swing += (target_swing - self.swing) * top;
                self.sway += (target_sway - self.sway) * top;
                self.ripple += (target_ripple - self.ripple) * top;
                match prefs.motion {
                    // Rigid, like the vanilla cape: every segment the same.
                    Motion::Follow => {
                        for i in 0..n {
                            self.angle[i] = self.swing;
                            self.roll[i] = self.sway;
                        }
                    }
                    Motion::Cloth | Motion::Cloth3d => {
                        let bend = self.swing * BEND / n as f32;
                        let roll_gain = if prefs.motion == Motion::Cloth3d { 1.6 } else { 1.0 };
                        for i in 0..n {
                            let (up_a, up_r) = if i == 0 {
                                (self.swing, self.sway)
                            } else {
                                (self.angle[i - 1], self.roll[i - 1])
                            };
                            let goal_a = (up_a + bend).min(MAX_SWING + MAX_BEND);
                            let goal_r = (up_r * if i == 0 { 1.0 } else { 1.0 + (roll_gain - 1.0) / n as f32 })
                                .clamp(-MAX_ROLL * 1.6, MAX_ROLL * 1.6);
                            self.angle[i] += (goal_a - self.angle[i]) * link;
                            self.roll[i] += (goal_r - self.roll[i]) * link;
                        }
                    }
                }
            }
            for v in [&mut self.swing, &mut self.sway, &mut self.ripple] {
                snap(v);
            }
            for v in self.angle.iter_mut().chain(self.roll.iter_mut()) {
                snap(v);
            }
            // The wave travels down while moving; it fades with the ripple.
            self.phase = (self.phase + dt * (0.35 + 0.45 * moving)) % std::f32::consts::TAU;
        }

        /// The centre line: one point per vertex row, in cape space (x right,
        /// y down from the collar, z out from the back), with each row's
        /// outward unit normal (shared by the two segments that meet there).
        fn spine(&self) -> Vec<([f32; 3], [f32; 3])> {
            let n = self.segments;
            let seg = CAPE_H / n as f32;
            let angles = self.segment_angles();
            let mut pts = Vec::with_capacity(n + 1);
            let (mut y, mut z, mut x) = (0.0f32, 0.0f32, 0.0f32);
            for i in 0..=n {
                // The row's normal: the average of the segments meeting there.
                let a = match i {
                    0 => angles[0].0,
                    i if i == n => angles[n - 1].0,
                    i => (angles[i - 1].0 + angles[i].0) * 0.5,
                }
                .to_radians();
                pts.push(([x, y, z.max(0.0)], [0.0, -a.sin(), a.cos()]));
                if i < n {
                    let (a, r) = angles[i];
                    let (a, r) = (a.to_radians(), r.to_radians());
                    y += seg * a.cos() * r.cos();
                    z += seg * a.sin();
                    x += seg * a.cos() * r.sin();
                }
            }
            pts
        }

        /// The cape as a closed 1-pixel-thick box bent along its segments:
        /// per segment an outer face (the picture's front, 1..11 x 1..17),
        /// an inner face (its back, 12..22, mirrored as vanilla's box is),
        /// and the two side edges (0..1 and 11..12), plus the top (1..11 x
        /// 0..1) and bottom (11..21 x 0..1) edges. Rows share vertices, so
        /// there are no gaps; each segment shows its own slice of the
        /// picture. `collar` is where the cape's top centre sits in the
        /// player's frame (feet, body yaw, +x left, +z forward).
        pub fn mesh(&self, collar: [f32; 3]) -> Vec<Face> {
            let n = self.segments;
            let spine = self.spine();
            let half_t = CAPE_T * 0.5;
            let hw = CAPE_W * 0.5;
            // Cape space → player frame: x right → -x, y down → -y, z out
            // (behind the player) → -z.
            let map = |p: [f32; 3]| [collar[0] - p[0], collar[1] - p[1], collar[2] - p[2]];
            let at = |row: usize, side: f32, out: f32| {
                let (c, nrm) = spine[row];
                map([
                    c[0] + side * hw,
                    c[1] + nrm[1] * out * half_t,
                    c[2] + nrm[2] * out * half_t,
                ])
            };
            let v_at = |row: usize| 1.0 + 16.0 * row as f32 / n as f32;
            let mut faces = Vec::with_capacity(n * 4 + 2);
            for i in 0..n {
                let (v0, v1) = (v_at(i), v_at(i + 1));
                let seg_out = {
                    let a = (spine[i].1, spine[i + 1].1);
                    [0.0, -(a.0[1] + a.1[1]), -(a.0[2] + a.1[2])]
                };
                // Outer face: the picture's front, left edge texel 1 at -x.
                faces.push(orient(
                    [at(i, -1.0, 1.0), at(i + 1, -1.0, 1.0), at(i + 1, 1.0, 1.0), at(i, 1.0, 1.0)],
                    uv_px(1.0, v0, 11.0, v1),
                    seg_out,
                ));
                // Inner face: the back, mirrored (texel 22 at -x, 12 at +x).
                faces.push(orient(
                    [at(i, -1.0, -1.0), at(i + 1, -1.0, -1.0), at(i + 1, 1.0, -1.0), at(i, 1.0, -1.0)],
                    uv_px(22.0, v0, 12.0, v1),
                    [0.0, -seg_out[1], -seg_out[2]],
                ));
                // Side edges: 0..1 beside the front's left, 11..12 its right.
                faces.push(orient(
                    [at(i, -1.0, 1.0), at(i + 1, -1.0, 1.0), at(i + 1, -1.0, -1.0), at(i, -1.0, -1.0)],
                    uv_px(1.0, v0, 0.0, v1),
                    [1.0, 0.0, 0.0],
                ));
                faces.push(orient(
                    [at(i, 1.0, 1.0), at(i + 1, 1.0, 1.0), at(i + 1, 1.0, -1.0), at(i, 1.0, -1.0)],
                    uv_px(11.0, v0, 12.0, v1),
                    [-1.0, 0.0, 0.0],
                ));
            }
            // Top edge (1..11 x 0..1, front side at v 1) and bottom edge
            // (11..21 x 0..1).
            faces.push(orient(
                [at(0, -1.0, -1.0), at(0, -1.0, 1.0), at(0, 1.0, 1.0), at(0, 1.0, -1.0)],
                uv_px(1.0, 0.0, 11.0, 1.0),
                [0.0, 1.0, 0.0],
            ));
            let down = {
                let (a, b) = (map(spine[n - 1].0), map(spine[n].0));
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
            };
            faces.push(orient(
                [at(n, -1.0, 1.0), at(n, -1.0, -1.0), at(n, 1.0, -1.0), at(n, 1.0, 1.0)],
                uv_px(11.0, 0.0, 21.0, 1.0),
                down,
            ));
            faces
        }
    }

    /// Normal of a face as the renderer computes it: (b - a) x (c - a).
    pub fn face_normal(c: &[[f32; 3]; 4]) -> [f32; 3] {
        let sub = |p: [f32; 3], q: [f32; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
        let (u, v) = (sub(c[1], c[0]), sub(c[2], c[0]));
        [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
    }

    /// Orders a face so it faces `outward` (player frame), keeping each
    /// corner's texel: reversing the corners swaps u0 and u1 with them.
    fn orient(c: [[f32; 3]; 4], uv: [f32; 4], outward: [f32; 3]) -> Face {
        let nrm = face_normal(&c);
        let dot = nrm[0] * outward[0] + nrm[1] * outward[1] + nrm[2] * outward[2];
        if dot >= 0.0 {
            (c, uv)
        } else {
            ([c[3], c[2], c[1], c[0]], [uv[2], uv[1], uv[0], uv[3]])
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub struct ClothPrefs {
        pub style: Style,
        pub wind: Wind,
        pub motion: Motion,
        pub quality: Quality,
        pub gravity: u8,
        pub wind_strength: u8,
    }

    impl ClothPrefs {
        pub fn from_prefs(p: &super::persist::Prefs) -> Self {
            Self {
                style: p.style,
                wind: p.wind,
                // The one cloth motion (the old "Cloth 3D").
                motion: Motion::Cloth3d,
                quality: p.quality,
                gravity: p.gravity,
                wind_strength: p.wind_strength,
            }
        }
    }
}

pub mod image {
    /// Default time on each sprite-sheet frame when the provider does not
    /// name a delay (GIF frames keep their own Graphic Control time).
    pub const DEFAULT_DELAY_MS: u16 = 100;
    pub const MAX_FRAMES: usize = 64;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct PicFrame {
        pub width: u32,
        pub height: u32,
        pub rgba: Vec<u8>,
        pub delay_ms: u16,
    }

    /// OptiFine cape pictures are 46x22 (the vanilla 64x32 layout cropped to
    /// the cape and elytra) or a whole multiple of it (92x44, 184x88, ...).
    /// Returns the multiple, or `None` for any other size.
    pub fn optifine_scale(w: u32, h: u32) -> Option<u32> {
        if w == 0 || h == 0 || w % 46 != 0 || h % 22 != 0 {
            return None;
        }
        let s = w / 46;
        (h / 22 == s).then_some(s)
    }

    /// Brings an HD OptiFine cape (a multiple of 46x22 larger than the
    /// 64x32 slot) back to its 46x22 pixel grid by averaging each s x s
    /// block, so the cape slot keeps the vanilla layout 1:1 and the 22x17
    /// cape region (front at 1..11 x 1..17) lands where the cape box reads
    /// it. Without this the slot code stretches the whole picture to 64x32
    /// and the cape is misaligned. Any other picture is returned unchanged.
    pub fn fit_cape(w: u32, h: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
        let len = (w as usize) * (h as usize) * 4;
        let scale = optifine_scale(w, h).filter(|s| *s > 1 && rgba.len() >= len);
        let Some(s) = scale else {
            return (w, h, rgba.to_vec());
        };
        let (ow, oh) = (w / s, h / s);
        let mut out = vec![0u8; (ow * oh * 4) as usize];
        let n = (s * s) as u32;
        for y in 0..oh {
            for x in 0..ow {
                let mut acc = [0u32; 4];
                for dy in 0..s {
                    for dx in 0..s {
                        let i = (((y * s + dy) * w + (x * s + dx)) * 4) as usize;
                        for k in 0..4 {
                            acc[k] += u32::from(rgba[i + k]);
                        }
                    }
                }
                let o = ((y * ow + x) * 4) as usize;
                for k in 0..4 {
                    out[o + k] = ((acc[k] + n / 2) / n) as u8;
                }
            }
        }
        (ow, oh, out)
    }

    /// First frame of a vertical sprite sheet, kept for call sites that
    /// only need a still. Prefer [`split_sheet`] when animating.
    pub fn first_frame(w: u32, h: u32, rgba: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        let frames = split_sheet(w, h, rgba, false, None)?;
        let f = frames.into_iter().next()?;
        Some((f.width, f.height, f.rgba))
    }

    /// Slice a tall cape picture into stacked frames. A forced sheet
    /// (MinecraftCapes `animatedCape`) always uses the 2:1 convention
    /// (frame height = width / 2). Otherwise a strip is only treated as
    /// animated when it holds at least three standard frames, so a single
    /// 64×64 still is not split in two.
    pub fn split_sheet(
        w: u32,
        h: u32,
        rgba: &[u8],
        forced_sheet: bool,
        delay_ms: Option<u16>,
    ) -> Option<Vec<PicFrame>> {
        if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
            return None;
        }
        let fh = frame_height(w, h, forced_sheet);
        let n = (h / fh).clamp(1, MAX_FRAMES as u32) as usize;
        let delay = delay_ms.unwrap_or(DEFAULT_DELAY_MS).max(20);
        let mut out = Vec::with_capacity(n);
        let stride = (w * 4) as usize;
        let row_bytes = stride * fh as usize;
        for i in 0..n {
            let start = i * row_bytes;
            let end = start + row_bytes;
            if end > rgba.len() {
                break;
            }
            out.push(PicFrame {
                width: w,
                height: fh,
                rgba: rgba[start..end].to_vec(),
                delay_ms: delay,
            });
        }
        if out.is_empty() {
            return None;
        }
        Some(out)
    }

    fn frame_height(w: u32, h: u32, forced: bool) -> u32 {
        if w == 22 && h % 17 == 0 && (forced || h >= 17 * 3) {
            return 17;
        }
        if w >= 2 {
            let half = w / 2;
            if h % half == 0 && (forced || h >= half * 3) {
                return half;
            }
        }
        h
    }

    pub fn any_visible(rgba: &[u8]) -> bool {
        rgba.chunks(4).any(|p| p.len() == 4 && p[3] > 8)
    }

    /// Which frame is showing after `elapsed_ms`. Off or a still always
    /// returns 0. A zero delay is treated as [`DEFAULT_DELAY_MS`].
    pub fn frame_index(delays: &[u16], elapsed_ms: u64, animated: bool) -> usize {
        if !animated || delays.len() <= 1 {
            return 0;
        }
        let cycle: u64 = delays
            .iter()
            .map(|d| u64::from(if *d == 0 { DEFAULT_DELAY_MS } else { *d }))
            .sum();
        if cycle == 0 {
            return 0;
        }
        let mut left = elapsed_ms % cycle;
        for (i, d) in delays.iter().enumerate() {
            let step = u64::from(if *d == 0 { DEFAULT_DELAY_MS } else { *d });
            if left < step {
                return i;
            }
            left -= step;
        }
        delays.len() - 1
    }

    /// A from-scratch GIF87a / GIF89a decoder (LZW). The `gif` crate is in
    /// the client's workspace lockfile but is not a client dependency,
    /// so it cannot be used under `--locked`.
    pub fn decode_gif(bytes: &[u8]) -> Option<Vec<PicFrame>> {
        gif::decode(bytes)
    }

    pub fn is_gif(bytes: &[u8]) -> bool {
        bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")
    }

    mod gif {
        use super::{PicFrame, DEFAULT_DELAY_MS, MAX_FRAMES};

        pub fn decode(bytes: &[u8]) -> Option<Vec<PicFrame>> {
            if bytes.len() < 13 {
                return None;
            }
            if !bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a") {
                return None;
            }
            let mut i = 6;
            let sw = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
            let sh = u16::from_le_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            i += 4;
            if sw == 0 || sh == 0 || sw > 512 || sh > 2048 {
                return None;
            }
            let packed = bytes[i];
            let bg = bytes[i + 1];
            i += 3;
            let gct_flag = packed & 0x80 != 0;
            let gct_size = if gct_flag { 1 << ((packed & 7) + 1) } else { 0 };
            let gct = if gct_flag {
                let n = gct_size * 3;
                let t = bytes.get(i..i + n)?.to_vec();
                i += n;
                Some(t)
            } else {
                None
            };
            let mut canvas = vec![0u8; sw * sh * 4];
            if let Some(gct) = gct.as_ref() {
                fill_index(&mut canvas, sw, sh, gct, bg);
            }
            let mut delay_cs: u16 = 10;
            let mut trans: Option<u8> = None;
            let mut disposal: u8 = 0;
            let mut out: Vec<PicFrame> = Vec::new();
            let mut previous: Option<Vec<u8>> = None;
            while i < bytes.len() {
                match bytes[i] {
                    0x3B => break,
                    0x21 => {
                        i += 1;
                        let label = *bytes.get(i)?;
                        i += 1;
                        if label == 0xF9 {
                            let block = read_subblocks(bytes, &mut i)?;
                            if !block.is_empty() {
                                let p = block[0];
                                disposal = (p >> 2) & 0x07;
                                trans = if p & 1 != 0 { block.get(3).copied() } else { None };
                                delay_cs = if block.len() >= 3 {
                                    u16::from_le_bytes([block[1], block[2]])
                                } else {
                                    10
                                };
                            }
                        } else {
                            let _ = read_subblocks(bytes, &mut i)?;
                        }
                    }
                    0x2C => {
                        i += 1;
                        if i + 9 > bytes.len() {
                            return None;
                        }
                        let left = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
                        let top = u16::from_le_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
                        let iw = u16::from_le_bytes([bytes[i + 4], bytes[i + 5]]) as usize;
                        let ih = u16::from_le_bytes([bytes[i + 6], bytes[i + 7]]) as usize;
                        let ip = bytes[i + 8];
                        i += 9;
                        if iw == 0 || ih == 0 {
                            return None;
                        }
                        let lct_flag = ip & 0x80 != 0;
                        let interlace = ip & 0x40 != 0;
                        let lct = if lct_flag {
                            let n = (1 << ((ip & 7) + 1)) * 3;
                            let t = bytes.get(i..i + n)?.to_vec();
                            i += n;
                            Some(t)
                        } else {
                            None
                        };
                        let table = lct.as_ref().or(gct.as_ref())?;
                        let min_code = *bytes.get(i)?;
                        i += 1;
                        let data = read_subblocks(bytes, &mut i)?;
                        let indices = lzw_decode(min_code, &data, iw * ih)?;
                        let before = if disposal == 3 {
                            Some(canvas.clone())
                        } else {
                            None
                        };
                        if disposal != 3 {
                            previous = Some(canvas.clone());
                        }
                        blit(
                            &mut canvas,
                            sw,
                            sh,
                            &indices,
                            iw,
                            ih,
                            left,
                            top,
                            table,
                            trans,
                            interlace,
                        );
                        let mut delay_ms = u32::from(delay_cs) * 10;
                        if delay_ms == 0 {
                            delay_ms = u32::from(DEFAULT_DELAY_MS);
                        }
                        out.push(PicFrame {
                            width: sw as u32,
                            height: sh as u32,
                            rgba: canvas.clone(),
                            delay_ms: (delay_ms as u16).clamp(20, 60_000),
                        });
                        if out.len() >= MAX_FRAMES {
                            break;
                        }
                        match disposal {
                            2 => {
                                if let Some(gct) = gct.as_ref() {
                                    fill_index(&mut canvas, sw, sh, gct, bg);
                                } else {
                                    canvas.fill(0);
                                }
                            }
                            3 => {
                                if let Some(prev) = before.or(previous.clone()) {
                                    canvas = prev;
                                }
                            }
                            _ => {}
                        }
                        delay_cs = 10;
                        trans = None;
                        disposal = 0;
                    }
                    _ => return None,
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(out)
            }
        }

        fn fill_index(canvas: &mut [u8], w: usize, h: usize, table: &[u8], index: u8) {
            let i = index as usize * 3;
            let (r, g, b) = if i + 2 < table.len() {
                (table[i], table[i + 1], table[i + 2])
            } else {
                (0, 0, 0)
            };
            for y in 0..h {
                for x in 0..w {
                    let o = (y * w + x) * 4;
                    canvas[o] = r;
                    canvas[o + 1] = g;
                    canvas[o + 2] = b;
                    canvas[o + 3] = 255;
                }
            }
        }

        fn blit(
            canvas: &mut [u8],
            sw: usize,
            sh: usize,
            indices: &[u8],
            iw: usize,
            ih: usize,
            left: usize,
            top: usize,
            table: &[u8],
            trans: Option<u8>,
            interlace: bool,
        ) {
            let rows = interlace_rows(ih, interlace);
            for (src_y, dest_y) in rows.into_iter().enumerate() {
                if dest_y >= ih {
                    continue;
                }
                let cy = top + dest_y;
                if cy >= sh {
                    continue;
                }
                for x in 0..iw {
                    let cx = left + x;
                    if cx >= sw {
                        continue;
                    }
                    let Some(&idx) = indices.get(src_y * iw + x) else {
                        continue;
                    };
                    if trans == Some(idx) {
                        continue;
                    }
                    let ti = idx as usize * 3;
                    if ti + 2 >= table.len() {
                        continue;
                    }
                    let o = (cy * sw + cx) * 4;
                    canvas[o] = table[ti];
                    canvas[o + 1] = table[ti + 1];
                    canvas[o + 2] = table[ti + 2];
                    canvas[o + 3] = 255;
                }
            }
        }

        fn interlace_rows(h: usize, interlace: bool) -> Vec<usize> {
            if !interlace {
                return (0..h).collect();
            }
            let mut rows = Vec::with_capacity(h);
            for (start, step) in [(0, 8), (4, 8), (2, 4), (1, 2)] {
                let mut y = start;
                while y < h {
                    rows.push(y);
                    y += step;
                }
            }
            rows
        }

        fn read_subblocks(bytes: &[u8], i: &mut usize) -> Option<Vec<u8>> {
            let mut out = Vec::new();
            loop {
                let n = *bytes.get(*i)? as usize;
                *i += 1;
                if n == 0 {
                    return Some(out);
                }
                out.extend_from_slice(bytes.get(*i..*i + n)?);
                *i += n;
            }
        }

        fn lzw_decode(min_code: u8, data: &[u8], pixels: usize) -> Option<Vec<u8>> {
            if !(2..=8).contains(&min_code) {
                return None;
            }
            let clear = 1u16 << min_code;
            let eoi = clear + 1;
            let mut code_bits = min_code as u32 + 1;
            let mut next = eoi + 1;
            let mut dict: Vec<(u16, u8)> = Vec::with_capacity(4096);
            let reset = |dict: &mut Vec<(u16, u8)>, next: &mut u16, bits: &mut u32| {
                dict.clear();
                for i in 0..clear {
                    dict.push((u16::MAX, i as u8));
                }
                dict.push((u16::MAX, 0));
                dict.push((u16::MAX, 0));
                *next = eoi + 1;
                *bits = min_code as u32 + 1;
            };
            reset(&mut dict, &mut next, &mut code_bits);
            let mut bits = BitReader { data, i: 0, buf: 0, n: 0 };
            let mut out = Vec::with_capacity(pixels);
            let mut prev: Option<u16> = None;
            loop {
                if out.len() >= pixels {
                    break;
                }
                let code = bits.read(code_bits)?;
                if code == clear {
                    reset(&mut dict, &mut next, &mut code_bits);
                    prev = None;
                    continue;
                }
                if code == eoi {
                    break;
                }
                if (code as usize) > dict.len() {
                    return None;
                }
                let mut seq = if (code as usize) < dict.len() {
                    sequence(&dict, code)?
                } else if code == next && prev.is_some() {
                    let mut s = sequence(&dict, prev.unwrap())?;
                    s.push(s[0]);
                    s
                } else {
                    return None;
                };
                if seq.is_empty() {
                    return None;
                }
                let first = seq[0];
                out.append(&mut seq);
                if let Some(p) = prev {
                    if (next as usize) < 4096 {
                        dict.push((p, first));
                        next += 1;
                        if next == (1 << code_bits) && code_bits < 12 {
                            code_bits += 1;
                        }
                    }
                }
                prev = Some(code);
            }
            if out.len() < pixels {
                out.resize(pixels, 0);
            }
            out.truncate(pixels);
            Some(out)
        }

        fn sequence(dict: &[(u16, u8)], mut code: u16) -> Option<Vec<u8>> {
            let mut stack = Vec::new();
            let mut guard = 0;
            loop {
                if guard > 4096 {
                    return None;
                }
                guard += 1;
                let (pref, suf) = *dict.get(code as usize)?;
                stack.push(suf);
                if pref == u16::MAX {
                    break;
                }
                code = pref;
            }
            stack.reverse();
            Some(stack)
        }

        struct BitReader<'a> {
            data: &'a [u8],
            i: usize,
            buf: u32,
            n: u32,
        }

        impl BitReader<'_> {
            fn read(&mut self, width: u32) -> Option<u16> {
                while self.n < width {
                    let b = *self.data.get(self.i)?;
                    self.i += 1;
                    self.buf |= (b as u32) << self.n;
                    self.n += 8;
                }
                let v = (self.buf & ((1 << width) - 1)) as u16;
                self.buf >>= width;
                self.n -= width;
                Some(v)
            }
        }
    }
}

pub mod layout {
    pub const MARGIN: f32 = 8.0;
    pub const PAD: f32 = 10.0;
    pub const TITLE_TOP: f32 = 8.0;
    pub const TITLE_H: f32 = 20.0;
    pub const TAB_H: f32 = 18.0;
    pub const TAB_GAP: f32 = 2.0;
    pub const CHIP_H: f32 = 16.0;
    pub const CHIP_GAP: f32 = 2.0;
    pub const ROW: f32 = 18.0;
    pub const DONE_H: f32 = 20.0;
    pub const PREF_W: f32 = 320.0;
    pub const PREF_H: f32 = 240.0;

    #[derive(Clone, Debug)]
    pub struct Layout {
        pub panel: [f32; 4],
        pub title: [f32; 4],
        pub tabs: [f32; 4],
        pub body: [f32; 4],
        pub done: [f32; 4],
        pub tab: [f32; 4],
        pub n_tabs: usize,
    }

    impl Layout {
        pub fn tab_rect(&self, i: usize) -> [f32; 4] {
            let n = self.n_tabs.max(1);
            let w = ((self.tabs[2] - TAB_GAP * (n as f32 - 1.0)) / n as f32).max(24.0);
            [self.tabs[0] + i as f32 * (w + TAB_GAP), self.tabs[1], w, self.tabs[3]]
        }
    }

    pub fn compute(sw: f32, sh: f32, n_tabs: usize) -> Layout {
        let w = PREF_W.min((sw - MARGIN * 2.0).max(1.0));
        let h = PREF_H.min((sh - MARGIN * 2.0).max(1.0));
        let x = ((sw - w) * 0.5).floor().max(MARGIN);
        let y = ((sh - h) * 0.5).floor().max(MARGIN);
        let title = [x + PAD, y + TITLE_TOP, w - PAD * 2.0, TITLE_H];
        let tabs_y = title[1] + title[3] + 4.0;
        let tabs = [x + PAD, tabs_y, w - PAD * 2.0, TAB_H];
        let done = [x + PAD, y + h - PAD - DONE_H, w - PAD * 2.0, DONE_H];
        let body_y = tabs[1] + tabs[3] + 6.0;
        let body_h = (done[1] - 6.0 - body_y).max(40.0);
        let body = [x + PAD, body_y, w - PAD * 2.0, body_h];
        Layout {
            panel: [x, y, w, h],
            title,
            tabs,
            body,
            done,
            tab: [0.0; 4],
            n_tabs: n_tabs.max(1),
        }
    }

    pub fn chip_row(body: [f32; 4], index: usize) -> [f32; 4] {
        [
            body[0],
            body[1] + index as f32 * (CHIP_H + CHIP_GAP),
            body[2],
            CHIP_H,
        ]
    }

    pub fn source_row(body: [f32; 4], scroll: f32, index: usize) -> Option<[f32; 4]> {
        let y = body[1] + index as f32 * ROW - scroll;
        if y + ROW <= body[1] || y >= body[1] + body[3] {
            return None;
        }
        Some([body[0], y, body[2], ROW - 1.0])
    }

    /// One pause-menu button: (x, y, width, label key, action id, active),
    /// as the game's `pause_buttons` returns them.
    pub type PauseButton = (f32, f32, f32, &'static str, u8, bool);
    /// The pause action SuperCapes answers (unused by the game: it takes
    /// 0-15, 30, and 40 and up for added pages).
    pub const PAUSE_ACTION: u8 = 37;
    /// The label; not a translation key, so every language shows the name.
    pub const PAUSE_LABEL: &str = "SuperCapes";
    const PAUSE_ROW: f32 = 24.0;

    /// Adds the SuperCapes row, full width, directly above "Save and Quit to
    /// Title" / "Disconnect" (action 2), and moves that button (and anything
    /// below it) down one row. The rows above, server links and other mods'
    /// pages included, keep their places.
    pub fn insert_pause_row(buttons: &mut Vec<PauseButton>) {
        if buttons.iter().any(|b| b.4 == PAUSE_ACTION) {
            return;
        }
        let Some(at) = buttons.iter().position(|b| b.4 == 2) else {
            return;
        };
        let (x, y, w, ..) = buttons[at];
        for b in buttons.iter_mut() {
            if b.1 >= y - 0.5 {
                b.1 += PAUSE_ROW;
            }
        }
        buttons.insert(at, (x, y, w, PAUSE_LABEL, PAUSE_ACTION, true));
    }

    pub fn rect_inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
        inner[0] >= outer[0] - 0.5
            && inner[1] >= outer[1] - 0.5
            && inner[0] + inner[2] <= outer[0] + outer[2] + 0.5
            && inner[1] + inner[3] <= outer[1] + outer[3] + 0.5
    }
}

pub mod close {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Destination {
        Pause,
        World,
    }

    pub fn apply_take_over(claim: bool, want_close: bool) -> (bool, bool) {
        if want_close {
            (false, true)
        } else {
            (claim, false)
        }
    }

    pub fn auto_return_to_pause(already_claimed: bool, current: bool) -> bool {
        if already_claimed {
            current
        } else {
            current
        }
    }

    pub fn decide_close(claimed: bool, want_close: bool, return_to_pause: bool) -> Option<Destination> {
        if !want_close || !claimed {
            return None;
        }
        if return_to_pause {
            Some(Destination::Pause)
        } else {
            Some(Destination::World)
        }
    }

    pub fn should_drop_claim(claim: bool, still_on_our_screen: bool) -> bool {
        claim && !still_on_our_screen
    }
}

pub const TABS: &[&str] = &["Motion", "Sources", "About"];

/// Fitting text into the card, measured with the game's font width.
pub mod text {
    /// Word-wraps `text` to lines no wider than `max_w` as `width` measures
    /// them. A word wider than a whole line is broken between characters.
    /// Always at least one line (an empty one for empty text).
    pub fn wrap(text: &str, max_w: f32, width: &dyn Fn(&str) -> f32) -> Vec<String> {
        let max_w = max_w.max(1.0);
        let mut lines = Vec::new();
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let joined = if cur.is_empty() {
                word.to_string()
            } else {
                format!("{cur} {word}")
            };
            if width(&joined) <= max_w {
                cur = joined;
                continue;
            }
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            if width(word) <= max_w {
                cur = word.to_string();
                continue;
            }
            // A single word longer than the line: break it by characters.
            for ch in word.chars() {
                let mut next = cur.clone();
                next.push(ch);
                if width(&next) > max_w && !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                    cur.push(ch);
                } else {
                    cur = next;
                }
            }
        }
        if !cur.is_empty() || lines.is_empty() {
            lines.push(cur);
        }
        lines
    }

    /// `text` as is if it fits in `max_w`, else cut with "..." so it does.
    pub fn fit(text: &str, max_w: f32, width: &dyn Fn(&str) -> f32) -> String {
        if width(text) <= max_w {
            return text.to_string();
        }
        let mut out = String::new();
        for ch in text.chars() {
            let mut next = out.clone();
            next.push(ch);
            if width(&format!("{next}...")) > max_w {
                break;
            }
            out = next;
        }
        format!("{}...", out.trim_end())
    }
}
