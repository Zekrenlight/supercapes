// SPDX-License-Identifier: MIT
//! SuperCapes for Sunrise: cape cloth that moves with the player, and public cape pictures.
//!
//! The typed hooks here draw the settings card, take its input and the
//! `/supercapes` command, and draw the cloth. The game side (cape downloads,
//! atlas slots, each player's cape sprite) is the support module
//! `src/native/game.rs`, run once a frame by the `frame` mixin
//! (`sunrise-mod.toml`).

pub mod logic;
pub mod session;
mod panel;
#[cfg(test)]
mod rules_tests;

use logic::physics;
use mod_api::{
    BlockEntityDrawContext, ClientHost, CommandEvent, Flow, Hooks, MobRender, PlayerRender,
    RenderFramePreEvent, RenderPlayerPostEvent, ScreenKeyPressedPreEvent,
    ScreenMouseButtonPressedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, Text,
};
use session as hook;

#[derive(Default)]
struct SuperCapes;

fn say(host: &mut dyn ClientHost, line: &str) {
    let _ = host.chat_line(Text::literal(line));
}

fn handle_command(host: &mut dyn ClientHost, command: &str) -> bool {
    let c = command.trim();
    let Some(rest) = c
        .strip_prefix("/supercapes")
        .or_else(|| c.strip_prefix("supercapes"))
    else {
        return false;
    };
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return false;
    }
    match rest.trim() {
        "" | "panel" => {
            let open = host.mod_screen() == Some(hook::MOD_ID) || hook::revise(|s| s.overlay);
            if open {
                hook::request_close();
            } else if host.is_paused() {
                hook::open_overlay();
            } else if host.mod_screen_open(hook::MOD_ID) {
                hook::revise(|s| {
                    s.overlay = false;
                    s.return_to_pause = false;
                    s.want_close = false;
                });
            } else {
                let msg = "Open SuperCapes from the pause menu while a world is running.";
                hook::revise(|s| s.status = msg.into());
                say(host, msg);
            }
        }
        "help" => say(
            host,
            "SuperCapes: /supercapes panel opens the settings card. There is no keybind.",
        ),
        _ => say(host, "SuperCapes commands: panel, help."),
    }
    true
}

/// The vanilla cape box's top edge and depth, in the player's frame (feet,
/// body yaw; the player model's 15/16 scale): the collar at 24 model pixels,
/// the cape's middle 2.5 pixels behind the body's centre.
const COLLAR_Y: f32 = 1.5 * physics::MODEL_SCALE;
const CAPE_Z: f32 = -2.5 / 16.0 * physics::MODEL_SCALE;

/// The cloth for one player, in the player's frame (at its feet, turned with
/// its body, +x its left). The game side cancels the vanilla cape layer for
/// exactly the players this draws (`session::cloth_replaces_vanilla`).
fn draw_cloth(mob: &MobRender<'_>, who: &PlayerRender<'_>, draw: &mut dyn BlockEntityDrawContext) {
    if !hook::listen_render() || mob.kind != "minecraft:player" {
        return;
    }
    let Some((sprite, body)) = hook::cloth_target(mob.id, who.local) else {
        return;
    };
    let prefs = hook::prefs_snapshot();
    let sneaking = body.as_ref().is_some_and(|b| b.crouching);
    let gliding = body.as_ref().is_some_and(|b| b.fall_flying);
    let world = match &body {
        Some(b) => {
            let a = mob.partial;
            [0, 1, 2].map(|k| (b.previous[k] as f32) * (1.0 - a) + (b.position[k] as f32) * a)
        }
        None => [0.0, 0.0, 0.0],
    };
    let mut cloth = hook::cloth_for(mob.id, &prefs);
    cloth.step(
        world,
        mob.body_yaw,
        mob.age,
        sneaking,
        gliding,
        physics::ClothPrefs::from_prefs(&prefs),
    );
    // CapeLayer over a chestplate: translate(0, -0.053125, 0.06875) in model units.
    let plated = body
        .as_ref()
        .and_then(|b| b.chest.as_deref())
        .is_some_and(|c| c != hook::ELYTRA);
    let mut ay = COLLAR_Y;
    let mut az = CAPE_Z;
    if sneaking {
        // The crouching body drops its shoulders 3.2 model pixels (the cloth
        // tilts out with the leaning body: `physics::SNEAK_TILT`).
        ay -= 3.2 / 16.0 * physics::MODEL_SCALE;
    }
    if plated {
        ay -= 0.053125 * physics::MODEL_SCALE;
        az -= 0.06875 * physics::MODEL_SCALE;
    }
    let faces = cloth.mesh([0.0, ay, az]);
    hook::store_cloth(mob.id, cloth);
    for (corners, uv) in faces {
        if !draw.quad(&sprite, corners, uv, 0xFFFFFF) {
            return;
        }
    }
}

#[mod_api::sunrise_mod]
impl Hooks for SuperCapes {
    fn register(&self, registrar: &mut dyn mod_api::Registrar, _cx: &mod_api::RegistrationContext) {
        // The accent colour of supercapes's cards (Sunrise orange by default).
        let _ = registrar.register_setting(
            mod_api::SettingSpec::color("accent", "supercapes.setting.accent", 0xF9801D)
                .tooltip("supercapes.setting.accent.tooltip"),
        );
    }

    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        if handle_command(host, event.command) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    /// Applies a close the card asked for (Done, Escape, a click outside it).
    fn render_frame_pre(&self, host: &mut dyn ClientHost, _event: &mut RenderFramePreEvent) {
        if !hook::revise(|s| s.want_close) {
            return;
        }
        if hook::revise(|s| s.overlay) {
            hook::revise(|s| {
                s.overlay = false;
                s.want_close = false;
            });
            return;
        }
        if host.mod_screen() == Some(hook::MOD_ID) {
            host.mod_screen_close(hook::MOD_ID);
        }
        hook::revise(|s| s.want_close = false);
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        panel::set_accent(host.setting_color("accent").unwrap_or(0xF9801D));
        panel::paint(host, &mut *event.draw, event.screen);
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if panel::click(host, event.screen, event.button) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        if panel::wheel(host, event.screen, event.scroll) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if panel::key(host, event.screen, event.key) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn render_player_post(&self, _host: &dyn ClientHost, event: &mut RenderPlayerPostEvent<'_>) {
        draw_cloth(event.player, event.who, &mut *event.draw);
    }
}
