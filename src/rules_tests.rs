// SPDX-License-Identifier: MIT
// Logic tests (no game runtime): `cargo test --lib`.

use crate::logic as supercapes_logic;

use supercapes_logic::close::{
    apply_take_over, decide_close, should_drop_claim, Destination,
};
use supercapes_logic::layout::{
    chip_row, compute, insert_pause_row, rect_inside, source_row, PauseButton, PAUSE_ACTION,
    PAUSE_LABEL,
};
use supercapes_logic::physics::{
    self, face_normal, uv_px, Cloth, ClothPrefs, Face, Motion, Quality, Style, Wind,
};
use supercapes_logic::sources::{
    default_toggles, enabled_in_order, is_fetched_sprite, merge_toggles, minecraftcapes_animated,
    minecraftcapes_cape_url, move_source, official_cape, pick_cape, request_url, Kind,
};
use supercapes_logic::{dump_prefs, image, load_prefs, Prefs, TABS};

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

fn finite_rect(r: [f32; 4]) -> bool {
    r.iter().all(|v| v.is_finite())
}

#[test]
fn defaults_enable_optifine_and_minecraftcapes() {
    let p = Prefs::default();
    assert!(p.online);
    assert!(!p.only_self);
    assert!(p.elytra);
    assert!(p.animated);
    assert!(p.cloth(), "Smooth by default");
    assert_eq!(p.style, Style::Smooth);
    assert_eq!(p.wind, Wind::Waves);
    assert_eq!(p.quality, Quality::Medium);
    assert_eq!(p.gravity, 5);
    assert_eq!(p.wind_strength, 4);
    let on: Vec<_> = default_toggles()
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(on, vec!["optifine".to_string(), "minecraftcapes".to_string()]);
    let off: Vec<_> = default_toggles()
        .into_iter()
        .filter(|(_, on)| !*on)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(off, vec!["labymod".to_string(), "cloaks".to_string()]);
}

#[test]
fn prefs_round_trip_and_flags() {
    let mut p = Prefs::default();
    p.online = false;
    p.only_self = true;
    p.elytra = false;
    p.animated = false;
    p.gravity = 9;
    p.wind_strength = 2;
    p.wind = Wind::Off;
    p.quality = Quality::High;
    p.style = Style::Blocky;
    p.sources[0].1 = false;
    p.sources[2].1 = true;
    let text = dump_prefs(&p);
    assert!(text.contains("format=3"));
    assert!(!text.contains("cloth=") && !text.contains("motion="), "no old keys");
    assert!(text.contains("style=blocky"));
    assert!(text.contains("online=no"));
    assert!(text.contains("only_self=yes"));
    assert!(text.contains("elytra=no"));
    assert!(text.contains("animated=no"));
    assert!(text.contains("wind=off"));
    assert!(text.contains("quality=high"));
    assert!(text.contains("wind_strength=2"));
    assert!(text.contains("source.optifine=no"));
    assert!(text.contains("source.labymod=yes"));
    let loaded = load_prefs(&text);
    assert!(!loaded.online);
    assert!(loaded.only_self);
    assert!(!loaded.elytra);
    assert!(!loaded.animated);
    assert_eq!(loaded.gravity, 9);
    assert_eq!(loaded.wind_strength, 2);
    assert_eq!(loaded.wind, Wind::Off);
    assert_eq!(loaded.quality, Quality::High);
    assert_eq!(loaded.style, Style::Blocky);
    assert_eq!(loaded.sources, p.sources);
}

#[test]
fn prefs_ignore_unknown_and_keep_defaults() {
    let loaded = load_prefs("format=3\nunknown=yes\nsource.nope=yes\norder=cloaks,optifine\n");
    let ids: Vec<_> = loaded.sources.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["cloaks", "optifine", "minecraftcapes", "labymod"]);
}

#[test]
fn source_urls_and_order() {
    let uuid = 0x123e4567e89b12d3_a456426614174000u128;
    let of = request_url(Kind::OptiFine, "Steve", uuid);
    assert_eq!(of, "http://s.optifine.net/capes/Steve.png");
    let mcc = request_url(Kind::MinecraftCapes, "Steve", uuid);
    assert!(mcc.contains("123e4567e89b12d3a456426614174000"));
    assert!(!mcc.contains('-'));
    let laby = request_url(Kind::LabyMod, "Steve", uuid);
    assert!(laby.contains("00e8db8c-79b2-4158-88a7-7c2aa17121b3"));
    assert!(laby.contains("123e4567-e89b-12d3-a456-426614174000"));
    let cloaks = request_url(Kind::Cloaks, "Steve", uuid);
    assert!(cloaks.contains("123e4567-e89b-12d3-a456-426614174000"));

    let mut prefs = default_toggles();
    assert!(move_source(&mut prefs, "labymod", true));
    let ids: Vec<_> = prefs.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids[1], "labymod");
    prefs[1].1 = true;
    prefs[0].1 = false;
    let enabled = enabled_in_order(&prefs);
    assert_eq!(enabled[0].id, "labymod");
    assert_eq!(enabled[1].id, "minecraftcapes");
}

#[test]
fn minecraftcapes_json_parser() {
    let url = minecraftcapes_cape_url(
        r#"{"animatedCape":false,"cape_url":"https://example.com/c.png"}"#,
    );
    assert_eq!(url.as_deref(), Some("https://example.com/c.png"));
    assert!(minecraftcapes_cape_url(r#"{"cape_url":null}"#).is_none());
    assert!(minecraftcapes_cape_url(r#"{"cape_url":"ftp://x"}"#).is_none());
    assert!(minecraftcapes_animated(r#"{"animatedCape": true}"#));
    assert!(!minecraftcapes_animated(r#"{"animatedCape":false}"#));
    assert_eq!(
        supercapes_logic::sources::provider_frame_delay(r#"{"frameDelay":150}"#),
        Some(150)
    );
    assert_eq!(
        supercapes_logic::sources::provider_frame_delay(r#"{"frame_delay":8}"#),
        Some(80)
    );
}

#[test]
fn first_frame_and_visibility() {
    let mut rgba = vec![0u8; 8 * 8 * 4];
    rgba[0] = 255;
    rgba[3] = 255;
    let (fw, fh, out) = image::first_frame(8, 8, &rgba).unwrap();
    assert_eq!((fw, fh), (8, 8));
    assert!(image::any_visible(&out));
    assert!(!image::any_visible(&[0, 0, 0, 0, 10, 10, 10, 1]));
}

#[test]
fn sprite_sheet_splits_stacked_frames() {
    // 16×48 is three 16×8 (width/2) frames; a 16×16 still must not split.
    let w = 16u32;
    let fh = 8u32;
    let mut sheet = vec![0u8; (w * fh * 3 * 4) as usize];
    // Frame 1 is opaque red, frame 2 green, frame 3 blue.
    for f in 0..3u32 {
        let color = [((f == 0) as u8) * 255, ((f == 1) as u8) * 255, ((f == 2) as u8) * 255, 255];
        let start = (f * w * fh * 4) as usize;
        for px in sheet[start..start + (w * fh * 4) as usize].chunks_mut(4) {
            px.copy_from_slice(&color);
        }
    }
    let still = image::split_sheet(16, 16, &sheet[..16 * 16 * 4], false, None).unwrap();
    assert_eq!(still.len(), 1);
    assert_eq!(still[0].height, 16);

    let frames = image::split_sheet(w, fh * 3, &sheet, false, Some(150)).unwrap();
    assert_eq!(frames.len(), 3);
    assert!(frames.iter().all(|f| f.width == 16 && f.height == 8 && f.delay_ms == 150));
    assert_eq!(&frames[0].rgba[0..4], &[255, 0, 0, 255]);
    assert_eq!(&frames[1].rgba[0..4], &[0, 255, 0, 255]);
    assert_eq!(&frames[2].rgba[0..4], &[0, 0, 255, 255]);

    // Forced MinecraftCapes sheet: 16×16 is two frames even if it looks like a still.
    let forced = image::split_sheet(16, 16, &sheet[..16 * 16 * 4], true, None).unwrap();
    assert_eq!(forced.len(), 2);
    assert_eq!(forced[0].height, 8);
    assert_eq!(forced[0].delay_ms, image::DEFAULT_DELAY_MS);

    // Classic 22×17 stacked three high.
    let classic = vec![0u8; 22 * 17 * 3 * 4];
    let cframes = image::split_sheet(22, 17 * 3, &classic, false, None).unwrap();
    assert_eq!(cframes.len(), 3);
    assert_eq!(cframes[0].height, 17);
}

#[test]
fn frame_index_walks_delays() {
    let d = [50u16, 150, 100];
    assert_eq!(image::frame_index(&d, 0, true), 0);
    assert_eq!(image::frame_index(&d, 49, true), 0);
    assert_eq!(image::frame_index(&d, 50, true), 1);
    assert_eq!(image::frame_index(&d, 199, true), 1);
    assert_eq!(image::frame_index(&d, 200, true), 2);
    assert_eq!(image::frame_index(&d, 299, true), 2);
    assert_eq!(image::frame_index(&d, 300, true), 0);
    assert_eq!(image::frame_index(&d, 800, true), 2);
    assert_eq!(image::frame_index(&d, 80, false), 0);
    assert_eq!(image::frame_index(&[0, 0], 150, true), 1); // zero means default 100ms
    assert_eq!(image::frame_index(&[], 50, true), 0);
}

const GIF_TWO: &[u8] = &[
    71, 73, 70, 56, 57, 97, 2, 0, 2, 0, 241, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 33,
    249, 4, 4, 5, 0, 0, 0, 44, 0, 0, 0, 0, 2, 0, 2, 0, 0, 2, 3, 132, 161, 0, 0, 33, 249, 4, 4,
    20, 0, 0, 0, 44, 0, 0, 0, 0, 2, 0, 2, 0, 0, 2, 3, 140, 163, 0, 0, 59,
];
const GIF_PATTERN: &[u8] = &[
    71, 73, 70, 56, 57, 97, 4, 0, 1, 0, 241, 0, 0, 255, 0, 0, 0, 0, 255, 0, 0, 0, 0, 0, 0, 33,
    249, 4, 4, 10, 0, 0, 0, 44, 0, 0, 0, 0, 4, 0, 1, 0, 0, 2, 3, 68, 172, 0, 0, 59,
];

#[test]
fn gif_decoder_reads_frames_delays_and_pixels() {
    assert!(image::is_gif(GIF_TWO));
    let frames = image::decode_gif(GIF_TWO).expect("two-frame gif");
    assert_eq!(frames.len(), 2);
    assert_eq!((frames[0].width, frames[0].height), (2, 2));
    assert_eq!(frames[0].delay_ms, 50);
    assert_eq!(frames[1].delay_ms, 200);
    assert!(frames[0].rgba.chunks(4).all(|p| p == [255, 0, 0, 255]));
    assert!(frames[1].rgba.chunks(4).all(|p| p == [0, 0, 255, 255]));

    let pat = image::decode_gif(GIF_PATTERN).expect("pattern gif");
    assert_eq!(pat.len(), 1);
    assert_eq!(pat[0].width, 4);
    assert_eq!(&pat[0].rgba[0..4], &[255, 0, 0, 255]);
    assert_eq!(&pat[0].rgba[4..8], &[0, 0, 255, 255]);
    assert_eq!(&pat[0].rgba[8..12], &[255, 0, 0, 255]);
    assert_eq!(&pat[0].rgba[12..16], &[0, 0, 255, 255]);
    assert_eq!(pat[0].delay_ms, 100);
    assert!(image::decode_gif(b"not a gif").is_none());
}

fn prefs_with(motion: Motion, ripple: u8, weight: u8) -> ClothPrefs {
    ClothPrefs {
        style: Style::Smooth,
        wind: Wind::Waves,
        motion,
        quality: Quality::Medium,
        gravity: weight,
        wind_strength: ripple,
    }
}

/// The defaults the settings card starts with.
fn calm() -> ClothPrefs {
    ClothPrefs::from_prefs(&Prefs::default())
}

const COLLAR: [f32; 3] = [0.0, 1.40625, -0.146];

/// Runs `ticks` ticks at `fps` frames a tick along `path(t)` (position,
/// yaw), returning the cloth.
fn run(
    c: &mut Cloth,
    t0: f32,
    ticks: usize,
    prefs: ClothPrefs,
    path: impl Fn(f32) -> ([f32; 3], f32),
    mut each: impl FnMut(&Cloth),
) -> f32 {
    let mut t = t0;
    for _ in 0..ticks * 3 {
        t += 1.0 / 3.0;
        let (p, yaw) = path(t);
        c.step(p, yaw, t, false, false, prefs);
        each(c);
    }
    t
}

#[test]
fn cloth_stays_finite_and_pinned() {
    for motion in [Motion::Follow, Motion::Cloth, Motion::Cloth3d] {
        let mut cloth = Cloth::new(Quality::Medium, Style::Smooth);
        assert_eq!(cloth.segments, 16);
        // Sprinting, turning and falling at once.
        run(&mut cloth, 0.0, 40, prefs_with(motion, 9, 0), |t| ([t * 0.28, 64.0 - t * 0.3, 0.0], 270.0 + t * 9.0), |c| {
            let faces = c.mesh(COLLAR);
            assert!(faces.iter().all(|(p, uv)| p.iter().flatten().chain(uv.iter()).all(|v| v.is_finite())));
        });
        let (swing, sway, ripple) = cloth.angles();
        assert!(swing > 5.0, "moving lifts the cape ({swing})");
        assert!(swing <= physics::MAX_SWING && sway.abs() <= physics::MAX_ROLL);
        assert!(ripple <= physics::MAX_RIPPLE);
        for (a, _) in cloth.segment_angles() {
            assert!((0.0..=physics::MAX_SWING + physics::MAX_BEND).contains(&a));
        }
        // The top edge stays on the collar.
        let faces = cloth.mesh(COLLAR);
        let top = &faces[faces.len() - 2].0;
        for p in top {
            assert!((p[1] - COLLAR[1]).abs() < 0.04, "top edge on the collar");
        }
    }
    let mut c = Cloth::new(Quality::Medium, Style::Smooth);
    c.resize(Quality::Low, Style::Smooth);
    assert_eq!(c.segments, 8);
    c.resize(Quality::High, Style::Smooth);
    assert_eq!(c.segments, 24);
    assert_eq!(c.segment_angles().len(), 24);
}

#[test]
fn cloth_hangs_straight_down_at_rest() {
    // Never moved: no idle wind, the cape hangs straight from the start.
    let mut still = Cloth::new(Quality::High, Style::Smooth);
    run(&mut still, 0.0, 60, calm(), |_| ([10.0, 64.0, -3.0], 37.0), |c| {
        assert_eq!(c.angles(), (0.0, 0.0, 0.0), "no motion at rest");
        assert!(c.segment_angles().iter().all(|a| *a == (0.0, 0.0)));
    });
    // After a sprint, a fall and a turn, standing still settles back to 0.
    for motion in [Motion::Follow, Motion::Cloth, Motion::Cloth3d] {
        let mut c = Cloth::new(Quality::Medium, Style::Smooth);
        let prefs = prefs_with(motion, 9, 0);
        let t = run(&mut c, 0.0, 30, prefs, |t| ([t * 0.3, 70.0 - t * 0.5, 0.0], 90.0 + t * 6.0), |_| {});
        assert!(c.angles().0 > 1.0);
        run(&mut c, t, 150, prefs, |_| ([9.0, 55.0, 0.0], 264.0), |_| {});
        assert_eq!(c.angles(), (0.0, 0.0, 0.0), "{motion:?} settles");
        assert!(c.segment_angles().iter().all(|(a, r)| *a == 0.0 && *r == 0.0), "{motion:?} every segment at 0");
        // Straight down: the outer face's corners line up under the collar.
        let faces = c.mesh(COLLAR);
        for (p, _) in faces.iter().step_by(4).take(c.segments) {
            for q in p {
                assert!((q[2] - (COLLAR[2] - physics::CAPE_T * 0.5)).abs() < 1e-4, "hangs straight ({})", q[2]);
            }
        }
        let bottom = faces.last().unwrap().0;
        for q in bottom {
            assert!((q[1] - (COLLAR[1] - physics::CAPE_H)).abs() < 1e-4, "full length below the collar");
        }
    }
}

#[test]
fn walking_makes_a_travelling_ripple_and_the_hem_bends() {
    let walk = |t: f32| ([0.0, 64.0, t * 0.216], 0.0);
    let mut c = Cloth::new(Quality::Medium, Style::Smooth);
    let t = run(&mut c, 0.0, 30, calm(), walk, |_| {});
    // Bottom segment over time: it moves up and down while walking.
    let n = c.segments;
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    let mut top_lo_hi = (f32::MAX, f32::MIN);
    let mut peaks = Vec::new();
    let mut prev = c.segment_angles();
    run(&mut c, t, 40, calm(), walk, |c| {
        let a = c.segment_angles();
        lo = lo.min(a[n - 1].0);
        hi = hi.max(a[n - 1].0);
        top_lo_hi = (top_lo_hi.0.min(a[0].0), top_lo_hi.1.max(a[0].0));
        // Where along the cape the wave crest (largest bump over the base
        // bend) is, frame by frame.
        let crest = (1..n)
            .max_by(|&i, &j| {
                let bump = |k: usize| a[k].0 - prev[k].0;
                bump(i).partial_cmp(&bump(j)).unwrap()
            })
            .unwrap();
        peaks.push(crest);
        prev = a;
    });
    assert!(hi - lo > 5.0, "the hem visibly ripples while walking ({lo}..{hi})");
    assert!(top_lo_hi.1 - top_lo_hi.0 < hi - lo, "the wave grows toward the hem");
    let a = c.segment_angles();
    let mean_top: f32 = a[..4].iter().map(|x| x.0).sum::<f32>() / 4.0;
    let mean_bottom: f32 = a[n - 4..].iter().map(|x| x.0).sum::<f32>() / 4.0;
    assert!(mean_top > 5.0 && mean_top < 45.0, "walking swing {mean_top}");
    assert!(mean_bottom > mean_top, "the hem bends out further");
    // The crest moves down the cape (it is a travelling wave, not a flap).
    let downs = peaks.windows(2).filter(|w| w[1] > w[0]).count();
    let ups = peaks.windows(2).filter(|w| w[1] < w[0]).count();
    assert!(downs > ups, "crest travels down ({downs} vs {ups})");
    // Ripple off: no wave; the cape still bends.
    let mut off = calm();
    off.wind = Wind::Off;
    let mut c = Cloth::new(Quality::Medium, Style::Smooth);
    run(&mut c, 0.0, 40, off, walk, |_| {});
    assert_eq!(c.angles().2, 0.0);
}

#[test]
fn falling_lifts_turning_swings_weight_damps() {
    let fall = |t: f32| ([0.0, 80.0 - t * 0.8, 0.0], 0.0);
    let mut c = Cloth::new(Quality::Medium, Style::Smooth);
    run(&mut c, 0.0, 20, calm(), fall, |_| {});
    assert!(c.angles().0 > 8.0, "falling lifts it ({})", c.angles().0);
    let spin = |t: f32| ([0.0, 64.0, 0.0], t * 15.0);
    let mut c = Cloth::new(Quality::Medium, Style::Smooth);
    run(&mut c, 0.0, 10, calm(), spin, |_| {});
    let rolls: Vec<f32> = c.segment_angles().iter().map(|x| x.1).collect();
    assert!(rolls.iter().all(|r| r.abs() > 1.0), "turning swings it sideways");
    assert!(rolls.iter().all(|r| r.abs() <= physics::MAX_ROLL * 1.6));
    let sprint = |t: f32| ([0.0, 64.0, t * 0.28], 0.0);
    let mut light = Cloth::new(Quality::Medium, Style::Smooth);
    let mut heavy = Cloth::new(Quality::Medium, Style::Smooth);
    run(&mut light, 0.0, 40, prefs_with(Motion::Cloth, 0, 0), sprint, |_| {});
    run(&mut heavy, 0.0, 40, prefs_with(Motion::Cloth, 0, 9), sprint, |_| {});
    assert!(light.angles().0 > heavy.angles().0 * 1.5, "weight damps the lift");
    // A 100-block teleport snaps the cloak instead of flinging the cape.
    let before = light.angles().0;
    light.step([100.0, 64.0, 100.0], 0.0, 41.0, false, false, calm());
    assert!(light.angles().0 <= before + 1.0);
}

fn texels(uv: [f32; 4]) -> [f32; 4] {
    [uv[0] / 16.0 * 64.0, uv[1] / 16.0 * 32.0, uv[2] / 16.0 * 64.0, uv[3] / 16.0 * 32.0]
}

/// Faces of a mesh, grouped: per segment outer, inner, left, right; then
/// top and bottom.
fn groups(faces: &[Face], n: usize) -> [Vec<Face>; 4] {
    let mut g: [Vec<Face>; 4] = Default::default();
    for (i, f) in faces[..n * 4].iter().enumerate() {
        g[i % 4].push(*f);
    }
    g
}

/// Checks that the faces' uv rectangles, stacked by v, tile exactly
/// `u_span` x `v_span` texels: no gap, no overlap, same columns throughout.
fn tiles_exactly(faces: &[Face], u_span: (f32, f32), v_span: (f32, f32)) {
    let mut rects: Vec<[f32; 4]> = faces
        .iter()
        .map(|(_, uv)| {
            let t = texels(*uv);
            [t[0].min(t[2]), t[1].min(t[3]), t[0].max(t[2]), t[1].max(t[3])]
        })
        .collect();
    rects.sort_by(|a, b| a[1].partial_cmp(&b[1]).unwrap());
    let eps = 1e-4;
    for r in &rects {
        assert!((r[0] - u_span.0).abs() < eps && (r[2] - u_span.1).abs() < eps, "columns {r:?}");
        assert!(r[3] > r[1], "a slice has height");
    }
    assert!((rects[0][1] - v_span.0).abs() < eps, "starts at {}", v_span.0);
    assert!((rects.last().unwrap()[3] - v_span.1).abs() < eps, "ends at {}", v_span.1);
    for w in rects.windows(2) {
        assert!((w[0][3] - w[1][1]).abs() < eps, "no gap or overlap between {:?} and {:?}", w[0], w[1]);
    }
}

#[test]
fn cloth_mesh_uvs_cover_the_vanilla_cape_rectangles() {
    for quality in [Quality::Low, Quality::Medium, Quality::High] {
        for style in [Style::Smooth, Style::Blocky] {
            let mut c = Cloth::new(quality, style);
            run(&mut c, 0.0, 20, calm(), |t| ([0.0, 64.0, t * 0.28], t * 3.0), |_| {});
            let n = c.segments;
            let faces = c.mesh(COLLAR);
            assert_eq!(faces.len(), n * 4 + 2);
            let [outer, inner, left, right] = groups(&faces, n);
            // PlayerCapeModel's 10x16x1 box at texOffs(0, 0) on a 64x32 cape:
            // front 1..11 x 1..17, back 12..22, sides 0..1 and 11..12,
            // top 1..11 x 0..1, bottom 11..21 x 0..1.
            tiles_exactly(&outer, (1.0, 11.0), (1.0, 17.0));
            tiles_exactly(&inner, (12.0, 22.0), (1.0, 17.0));
            tiles_exactly(&left, (0.0, 1.0), (1.0, 17.0));
            tiles_exactly(&right, (11.0, 12.0), (1.0, 17.0));
            tiles_exactly(&faces[n * 4..n * 4 + 1], (1.0, 11.0), (0.0, 1.0));
            tiles_exactly(&faces[n * 4 + 1..], (11.0, 21.0), (0.0, 1.0));
            // Segment i shows slice i, top to bottom.
            for (i, (_, uv)) in outer.iter().enumerate() {
                let t = texels(*uv);
                let want = 1.0 + 16.0 * i as f32 / n as f32;
                assert!((t[1].min(t[3]) - want).abs() < 1e-4, "segment {i} slice");
            }
        }
    }
}

/// The texel u at each corner, as the draw call assigns them (corners 0
/// and 1 take u0, corners 2 and 3 take u1).
fn corner_u(f: &Face) -> [f32; 4] {
    let t = texels(f.1);
    [t[0], t[0], t[2], t[2]]
}

#[test]
fn cloth_mesh_faces_point_outward_and_share_edges() {
    let mut c = Cloth::new(Quality::Medium, Style::Smooth);
    run(&mut c, 0.0, 20, calm(), |t| ([0.0, 64.0 - t * 0.1, t * 0.25], t * 4.0), |_| {});
    let n = c.segments;
    let faces = c.mesh(COLLAR);
    let [outer, inner, left, right] = groups(&faces, n);
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // Player frame: +x the player's left, +z forward; the cape is behind.
    for f in &outer {
        assert!(dot(face_normal(&f.0), [0.0, 0.0, -1.0]) > 0.0, "outer faces away from the back");
        // Seen from behind, texel 1 (the picture's left) is on the viewer's
        // left, which is the player's left (+x).
        let u = corner_u(f);
        let x_at = |texel: f32| {
            let i = u.iter().position(|v| (v - texel).abs() < 1e-3).unwrap();
            f.0[i][0]
        };
        assert!(x_at(1.0) > x_at(11.0), "front not mirrored");
    }
    for f in &inner {
        assert!(dot(face_normal(&f.0), [0.0, 0.0, 1.0]) > 0.0, "inner faces the back");
        let u = corner_u(f);
        let i22 = u.iter().position(|v| (v - 22.0).abs() < 1e-3).unwrap();
        let i12 = u.iter().position(|v| (v - 12.0).abs() < 1e-3).unwrap();
        assert!(f.0[i22][0] > f.0[i12][0], "back mirrored as vanilla's box");
    }
    for f in &left {
        assert!(dot(face_normal(&f.0), [1.0, 0.0, 0.0]) > 0.0);
    }
    for f in &right {
        assert!(dot(face_normal(&f.0), [-1.0, 0.0, 0.0]) > 0.0);
    }
    assert!(face_normal(&faces[n * 4].0)[1] > 0.0, "top faces up");
    assert!(face_normal(&faces[n * 4 + 1].0)[1] < 0.0, "bottom faces down");
    // Adjacent segments share their edge vertices: no gaps or seams.
    let key = |p: &[f32; 3]| p.map(|v| (v * 1e4).round() as i64);
    for w in outer.windows(2).chain(inner.windows(2)) {
        let a: std::collections::BTreeSet<_> = w[0].0.iter().map(key).collect();
        let b: std::collections::BTreeSet<_> = w[1].0.iter().map(key).collect();
        assert_eq!(a.intersection(&b).count(), 2, "segments share an edge");
    }
    // The faces close the box: the outer and inner faces sit one model
    // pixel apart.
    for (o, i) in outer.iter().zip(&inner) {
        let co: [f32; 3] = o.0.iter().fold([0.0; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]);
        let ci: [f32; 3] = i.0.iter().fold([0.0; 3], |s, p| [s[0] + p[0], s[1] + p[1], s[2] + p[2]]);
        let d = ((co[0] - ci[0]).powi(2) + (co[1] - ci[1]).powi(2) + (co[2] - ci[2]).powi(2)).sqrt() / 4.0;
        assert!((d - physics::CAPE_T).abs() < 0.01, "thickness {d}");
    }
}

#[test]
fn cape_box_matches_vanilla() {
    // 10 x 16 x 1 model pixels at the player model's 15/16 scale.
    assert!((physics::CAPE_W - 0.5859375).abs() < 1e-6);
    assert!((physics::CAPE_H - 0.9375).abs() < 1e-6);
    assert!((physics::CAPE_T - 0.05859375).abs() < 1e-6);
    let c = Cloth::new(Quality::Medium, Style::Smooth);
    let faces = c.mesh(COLLAR);
    let xs: Vec<f32> = faces.iter().flat_map(|f| f.0.iter().map(|p| p[0])).collect();
    let w = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
    assert!((w - physics::CAPE_W).abs() < 1e-5);
}

#[test]
fn old_wind_settings_are_reset_to_calm_defaults() {
    let old = "format=1\nwind=waves\nmotion=cloth3d\ngravity=1\nwind_strength=9\nonline=no\n";
    let p = load_prefs(old);
    assert_eq!(p.style, Style::Smooth);
    assert_eq!(p.wind_strength, 4);
    assert_eq!(p.gravity, 5);
    assert!(!p.online, "non-motion settings are kept");
    assert_eq!(p.format, 3);
    // Format 1 had cloth off: still Blocky.
    assert_eq!(load_prefs("format=1\ncloth=no\n").style, Style::Blocky);
}

#[test]
fn cloth_and_motion_become_style() {
    let style = |text: &str| load_prefs(text).style;
    // Format 2 (the 1.0.0 test builds): cloth off or Follow looked rigid.
    assert_eq!(style("format=2\ncloth=no\nmotion=cloth3d\nstyle=smooth\n"), Style::Blocky);
    assert_eq!(style("format=2\ncloth=yes\nmotion=follow\nstyle=smooth\n"), Style::Blocky);
    assert_eq!(style("format=2\ncloth=no\nmotion=follow\n"), Style::Blocky);
    // Otherwise Smooth, whatever the old (inert) style said.
    assert_eq!(style("format=2\ncloth=yes\nmotion=cloth\nstyle=blocky\n"), Style::Smooth);
    assert_eq!(style("format=2\ncloth=yes\nmotion=cloth3d\n"), Style::Smooth);
    assert_eq!(style("format=2\n"), Style::Smooth);
    // Other settings survive the move.
    let p = load_prefs("format=2\ncloth=no\nwind_strength=7\ngravity=2\nelytra=no\nquality=high\n");
    assert_eq!((p.wind_strength, p.gravity, p.elytra, p.quality), (7, 2, false, Quality::High));
    // Format 3 reads Style as written and ignores stray old keys.
    assert_eq!(style("format=3\nstyle=blocky\n"), Style::Blocky);
    assert_eq!(style("format=3\nstyle=smooth\ncloth=no\nmotion=follow\n"), Style::Smooth);
    // Blocky turns the cloth off (the vanilla cape layer draws it).
    let mut p = Prefs::default();
    p.style = Style::Blocky;
    assert!(!p.cloth());
    let again = load_prefs(&dump_prefs(&p));
    assert_eq!(again.style, Style::Blocky);
    // The cloth always uses the one (old "Cloth 3D") motion.
    assert_eq!(ClothPrefs::from_prefs(&Prefs::default()).motion, Motion::Cloth3d);
}

fn solid(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            v.extend_from_slice(&f(x, y));
        }
    }
    v
}

#[test]
fn optifine_sizes_are_detected() {
    assert_eq!(image::optifine_scale(46, 22), Some(1));
    assert_eq!(image::optifine_scale(92, 44), Some(2));
    assert_eq!(image::optifine_scale(184, 88), Some(4));
    assert_eq!(image::optifine_scale(64, 32), None);
    assert_eq!(image::optifine_scale(92, 22), None);
    assert_eq!(image::optifine_scale(0, 0), None);
}

#[test]
fn optifine_hd_capes_map_onto_the_vanilla_cape_region() {
    const RED: [u8; 4] = [200, 20, 20, 255];
    const BLUE: [u8; 4] = [20, 20, 200, 255];
    for s in [1u32, 2, 4] {
        let (w, h) = (46 * s, 22 * s);
        // The front of the cape is 1..11 x 1..17 in 46x22 pixels.
        let front = |x: u32, y: u32| (1..11).contains(&(x / s)) && (1..17).contains(&(y / s));
        let pic = solid(w, h, |x, y| if front(x, y) { RED } else { BLUE });
        let (fw, fh, px) = image::fit_cape(w, h, &pic);
        assert_eq!((fw, fh), (46, 22), "scale {s}");
        for y in 0..22 {
            for x in 0..46 {
                let i = ((y * 46 + x) * 4) as usize;
                let want = if (1..11).contains(&x) && (1..17).contains(&y) { RED } else { BLUE };
                assert_eq!(&px[i..i + 4], &want, "scale {s} at {x},{y}");
            }
        }
    }
    // The slot keeps a 46x22 picture 1:1 in a 64x32 layout (the cloth's
    // UVs are checked against that layout in `cloth_mesh_*`).
    assert_eq!(uv_px(64.0, 32.0, 0.0, 0.0), [16.0, 16.0, 0.0, 0.0]);
    // Other sizes pass through unchanged.
    let vanilla = solid(64, 32, |_, _| BLUE);
    let (w, h, same) = image::fit_cape(64, 32, &vanilla);
    assert_eq!((w, h), (64, 32));
    assert_eq!(same, vanilla);
    // HD averaging blends a 2x2 block.
    let mixed = solid(92, 44, |x, y| if (x + y) % 2 == 0 { [0, 0, 0, 255] } else { [200, 100, 50, 255] });
    let (_, _, px) = image::fit_cape(92, 44, &mixed);
    assert_eq!(&px[0..4], &[100, 50, 25, 255]);
}

#[test]
fn layout_fits_and_does_not_overlap() {
    for (sw, sh) in [(320.0, 240.0), (854.0, 480.0), (1920.0, 1080.0), (200.0, 160.0)] {
        let lay = compute(sw, sh, TABS.len());
        assert!(finite_rect(lay.panel));
        assert!(lay.panel[0] >= 0.0 && lay.panel[1] >= 0.0);
        assert!(lay.panel[0] + lay.panel[2] <= sw + 0.5);
        assert!(lay.panel[1] + lay.panel[3] <= sh + 0.5);
        assert!(rect_inside(lay.title, lay.panel));
        assert!(rect_inside(lay.tabs, lay.panel));
        assert!(rect_inside(lay.body, lay.panel));
        assert!(rect_inside(lay.done, lay.panel));
        let tabs: Vec<_> = (0..TABS.len()).map(|i| lay.tab_rect(i)).collect();
        for t in &tabs {
            assert!(rect_inside(*t, lay.panel));
            assert!(finite_rect(*t));
        }
        for i in 0..tabs.len() {
            for j in i + 1..tabs.len() {
                assert!(!overlaps(tabs[i], tabs[j]), "tabs {i} and {j} overlap");
            }
            assert!(!overlaps(tabs[i], lay.done));
            assert!(!overlaps(tabs[i], lay.title));
        }
        assert!(!overlaps(lay.title, lay.done));
        assert!(!overlaps(lay.body, lay.done));
        for i in 0..8 {
            let row = chip_row(lay.body, i);
            assert!(finite_rect(row));
        }
        let first = source_row(lay.body, 0.0, 0).unwrap();
        assert!(finite_rect(first));
    }
}

fn vanilla_pause(links: bool) -> Vec<PauseButton> {
    // Sunrise's pause_buttons at 854x480: x0 = 321, y0 = 78 (one extra row: 72).
    let (x0, y0, down) = if links { (321.0, 72.0, 24.0) } else { (321.0, 78.0, 0.0) };
    let mut v = vec![
        (x0 + 4.0, y0 + 50.0, 204.0, "menu.returnToGame", 0, true),
        (x0 + 4.0, y0 + 74.0, 98.0, "gui.advancements", 4, true),
        (x0 + 110.0, y0 + 74.0, 98.0, "gui.stats", 3, true),
    ];
    if links {
        v.push((x0 + 4.0, y0 + 122.0, 204.0, "menu.server_links", 11, true));
    }
    v.extend([
        (x0 + 4.0, y0 + 122.0 + down, 98.0, "menu.options", 1, true),
        (x0 + 110.0, y0 + 122.0 + down, 98.0, "options.worldOptions.button", 9, true),
        (x0 + 4.0, y0 + 146.0 + down, 204.0, "menu.returnToMenu", 2, true),
    ]);
    v
}

fn rect(b: &PauseButton) -> [f32; 4] {
    [b.0, b.1, b.2, 20.0]
}

#[test]
fn pause_row_sits_right_above_save_and_quit() {
    for links in [false, true] {
        let before = vanilla_pause(links);
        let mut after = before.clone();
        insert_pause_row(&mut after);
        assert_eq!(after.len(), before.len() + 1);
        let ours = after.iter().position(|b| b.4 == PAUSE_ACTION).unwrap();
        let quit = after.iter().position(|b| b.4 == 2).unwrap();
        assert_eq!(quit, ours + 1, "ours comes right before quit in focus order");
        let (o, q) = (after[ours], after[quit]);
        let old_quit = before.iter().find(|b| b.4 == 2).unwrap();
        assert_eq!(o.3, PAUSE_LABEL);
        assert_eq!((o.0, o.1, o.2), (old_quit.0, old_quit.1, 204.0), "full width, where quit was");
        assert_eq!(q.1, old_quit.1 + 24.0, "quit moves down one row");
        // Everything else (server links, options, the icon row above) stays.
        for b in &before {
            if b.4 != 2 {
                assert!(after.contains(b));
            }
        }
        // Icon row (report bugs etc.) at y0 + 98.
        let y0 = if links { 72.0 } else { 78.0 };
        let icons = [before[0].0, y0 + 98.0, 92.0, 20.0];
        for (i, a) in after.iter().enumerate() {
            assert!(!overlaps(rect(a), icons) || a.4 != PAUSE_ACTION);
            for b in after.iter().skip(i + 1) {
                assert!(!overlaps(rect(a), rect(b)), "{} overlaps {}", a.3, b.3);
            }
        }
        assert!(q.1 + 20.0 <= 480.0);
        // Running twice adds nothing more.
        let once = after.clone();
        insert_pause_row(&mut after);
        assert_eq!(after, once);
    }
}

#[test]
fn close_rules() {
    assert_eq!(apply_take_over(true, true), (false, true));
    assert_eq!(apply_take_over(true, false), (true, false));
    assert_eq!(
        decide_close(true, true, true),
        Some(Destination::Pause)
    );
    assert_eq!(
        decide_close(true, true, false),
        Some(Destination::World)
    );
    assert!(decide_close(false, true, true).is_none());
    assert!(should_drop_claim(true, false));
    assert!(!should_drop_claim(true, true));
}

#[test]
fn merge_toggles_fills_known_sources() {
    let mut on = std::collections::BTreeMap::new();
    on.insert("cloaks".into(), true);
    let merged = merge_toggles(Some(&["cloaks".into()]), &on);
    assert_eq!(merged[0], ("cloaks".into(), true));
    assert_eq!(merged.len(), 4);
}

#[test]
fn text_wraps_to_the_card_width() {
    // A fixed-width "font": 6 pixels a character.
    let w = |t: &str| t.chars().count() as f32 * 6.0;
    let line = "Online sources are only asked for public cape pictures.";
    for max in [60.0, 120.0, 200.0, 1000.0] {
        let lines = supercapes_logic::text::wrap(line, max, &w);
        assert!(lines.iter().all(|l| w(l) <= max), "{lines:?}");
        assert_eq!(lines.join(" "), line, "no words lost at {max}");
    }
    let long = supercapes_logic::text::wrap("Supercalifragilistic", 30.0, &w);
    assert!(long.iter().all(|l| w(l) <= 30.0));
    assert_eq!(long.concat(), "Supercalifragilistic");
    assert_eq!(supercapes_logic::text::wrap("", 50.0, &w), vec![String::new()]);
    let cut = supercapes_logic::text::fit("Animated capes: On", 60.0, &w);
    assert!(w(&cut) <= 60.0 && cut.ends_with("..."));
    assert_eq!(supercapes_logic::text::fit("On", 60.0, &w), "On");
}

#[test]
fn cloth_yields_to_vanilla_when_the_body_is_laid_over() {
    use crate::session::{self as hook, Body};

    let standing = Body::default();
    assert!(!standing.laid_over());

    let mut sleeping = Body::default();
    sleeping.sleeping = true;
    assert!(sleeping.laid_over());

    let mut swimming = Body::default();
    swimming.swimming = true;
    assert!(swimming.laid_over());

    hook::revise(|s| {
        *s = Default::default();
        s.ready = true;
        s.prefs.style = Style::Smooth;
        s.visible.insert(7, "capes/test".into());
        s.bodies.insert(7, standing);
    });
    assert!(hook::cloth_target(7, false).is_some());
    assert!(hook::cloth_replaces_vanilla(7, false));

    hook::revise(|s| {
        s.bodies.insert(7, sleeping);
    });
    assert!(
        hook::cloth_target(7, false).is_none(),
        "sleeping keeps the vanilla cape"
    );
    assert!(!hook::cloth_replaces_vanilla(7, false));

    hook::revise(|s| {
        s.bodies.insert(7, swimming);
    });
    assert!(
        hook::cloth_target(7, false).is_none(),
        "swimming, crawling and riptide keep the vanilla cape"
    );
    assert!(!hook::cloth_replaces_vanilla(7, false));

    let mut elytra = Body::default();
    elytra.chest = Some(hook::ELYTRA.into());
    hook::revise(|s| {
        s.bodies.insert(7, elytra);
    });
    assert!(hook::cloth_target(7, false).is_none());
    assert!(!hook::cloth_replaces_vanilla(7, false));

    hook::revise(|s| *s = Default::default());
}

#[test]
fn provider_toggles_hide_fetched_capes() {
    let mut p = Prefs::default();
    assert!(p.provider_allowed("optifine", true));
    assert!(p.provider_allowed("optifine", false));
    assert!(
        !p.provider_allowed("labymod", true),
        "LabyMod is off by default"
    );
    assert!(!p.provider_allowed("nope", true));

    p.online = false;
    assert!(!p.provider_allowed("optifine", true));
    assert!(!p.provider_allowed("optifine", false));

    p = Prefs::default();
    p.only_self = true;
    assert!(p.provider_allowed("optifine", true));
    assert!(
        !p.provider_allowed("optifine", false),
        "Only me hides others"
    );

    p = Prefs::default();
    p.sources
        .iter_mut()
        .find(|(id, _)| id == "optifine")
        .unwrap()
        .1 = false;
    assert!(!p.provider_allowed("optifine", true));
    assert!(p.provider_allowed("minecraftcapes", true));
}

#[test]
fn pick_cape_restores_the_official_one_when_the_provider_is_off() {
    assert!(is_fetched_sprite("capes/supercapes-ab-f0"));
    assert!(is_fetched_sprite("capes/supercapes-ab"));
    assert!(!is_fetched_sprite("capes/mojang-minecon"));
    assert!(!is_fetched_sprite("minecraft:textures/entity/cape.png"));

    let fetched = "capes/supercapes-optifine";
    let official = "capes/mojang-minecon";

    assert_eq!(
        pick_cape(Some(fetched), Some(official), None).as_deref(),
        Some(fetched),
        "an allowed provider wins"
    );
    assert_eq!(
        pick_cape(None, Some(fetched), Some(official)).as_deref(),
        Some(official),
        "a fetched sprite left on the mob is not kept"
    );
    assert_eq!(
        pick_cape(None, Some(fetched), None),
        None,
        "no official cape and no provider: hide the fetched one"
    );
    assert_eq!(
        pick_cape(None, Some(official), None).as_deref(),
        Some(official)
    );
    assert_eq!(
        official_cape(Some(fetched), Some(official)).as_deref(),
        Some(official)
    );
    assert_eq!(
        official_cape(Some(official), Some("capes/old")).as_deref(),
        Some(official),
        "a freshly refreshed official cape updates the memory"
    );
    assert_eq!(official_cape(Some(fetched), None), None);
    assert_eq!(
        official_cape(None, Some(official)).as_deref(),
        Some(official)
    );
}

#[test]
fn drop_disallowed_capes_honours_online_source_and_only_me() {
    use crate::session::{self as hook, CapeFrame, ReadyCape};

    let cape = |source: &str| ReadyCape {
        source: source.into(),
        frames: vec![CapeFrame {
            location: format!("capes/supercapes-{source}"),
            rgba: vec![0; 16],
            delay_ms: 100,
        }],
    };
    let local = 1u128;
    let other = 2u128;

    hook::revise(|s| {
        *s = Default::default();
        s.ready = true;
        s.prefs = Prefs::default();
        s.capes.insert(local, cape("optifine"));
        s.capes.insert(other, cape("optifine"));
    });

    hook::drop_disallowed_capes(Some(local));
    hook::revise(|s| {
        assert_eq!(s.capes.len(), 2, "defaults keep fetched capes");
    });

    hook::revise(|s| s.prefs.online = false);
    hook::drop_disallowed_capes(Some(local));
    hook::revise(|s| {
        assert!(s.capes.is_empty(), "Online off drops already-fetched capes");
        s.prefs.online = true;
        s.capes.insert(local, cape("optifine"));
        s.capes.insert(other, cape("minecraftcapes"));
        s.prefs
            .sources
            .iter_mut()
            .find(|(id, _)| id == "optifine")
            .unwrap()
            .1 = false;
    });
    hook::drop_disallowed_capes(Some(local));
    hook::revise(|s| {
        assert!(
            !s.capes.contains_key(&local),
            "OptiFine off drops that cape"
        );
        assert!(
            s.capes.contains_key(&other),
            "a still-enabled source is kept"
        );
        s.prefs = Prefs::default();
        s.prefs.only_self = true;
        s.capes.insert(local, cape("optifine"));
        s.capes.insert(other, cape("optifine"));
    });
    hook::drop_disallowed_capes(Some(local));
    hook::revise(|s| {
        assert!(s.capes.contains_key(&local));
        assert!(!s.capes.contains_key(&other), "Only me drops other players");
        s.capes.insert(other, cape("optifine"));
    });
    hook::drop_disallowed_capes(None);
    hook::revise(|s| {
        assert_eq!(
            s.capes.len(),
            2,
            "Only me keeps memory when the local UUID is not known yet"
        );
        *s = Default::default();
    });
}
