# SuperCapes

Version 1.0.0 · Minecraft Java 26.3 · Sunrise API 0.2 · Mod SDK `9425651` · MIT licence

SuperCapes is written in Rust for Sunrise. It needs no Java or Fabric.
It gives capes cloth physics that move with the player and shows public cape pictures for you and other players. A settings card controls it.

It is an independent implementation. It shares no code, assets, names or config formats with other cape mods.

<img width="639" height="477" alt="image" src="https://github.com/user-attachments/assets/57b8af28-5224-4bcf-b93a-c062a7f7de54" />
<img width="639" height="477" alt="image" src="https://github.com/user-attachments/assets/b47b9c47-bdc5-4de7-8255-5c8145839e60" />
<img width="639" height="477" alt="image" src="https://github.com/user-attachments/assets/a3a70cb4-47b6-4e68-8737-e9a121bb7f0d" />


## Network and privacy

**SuperCapes downloads public cape pictures.** With **Online capes** on (the default), it asks each enabled source for the cape of every player you can see. It sends only the player's name or UUID. **OptiFine** and **MinecraftCapes** are on by default. LabyMod and Cloaks are off.

- Requests go out from a background thread with the User-Agent `SuperCapes/1.0.0`.
- It sends no cookies, account tokens, telemetry, or machine or hardware IDs, hashed or otherwise.
- It has no author API, news, credits or announcements.
- Turn **Online capes** off and it makes no requests. **Only me** limits lookups to your own name.

## Features

- A settings card with Motion, Sources and About tabs. Open it with the **SuperCapes** button on the pause menu, directly above **Save and Quit to Title**, or with `/supercapes panel`. There is no extra keybind. The button is a regular game button, so resource packs restyle it.
- A segmented cape (WaveyCapes-style) driven only by how the player moves. Each segment follows the one above it, so walking sends a ripple down it and the hem bends further out. It lifts when you run or fall, swings sideways when you strafe or turn, and hangs straight down when you stand still. There is no wind. Settings:
  - style: **Smooth** (this segmented cloth, the default) or **Blocky** (the rigid vanilla cape, with vanilla physics)
  - for Smooth only (hidden when Blocky):
    - ripple: off, or a wave that runs along the cape only while you move
    - quality: Low, Medium or High (8, 16 or 24 segments)
    - weight: 0 to 9 (default 5); 0 lifts as far as the vanilla cape
    - ripple strength: 0 to 9 (default 4)
  - elytra uses cape: on (the winning cape picture on the wings) or off (a plain vanilla elytra, never the game's own cape)
- One cape per player. The first enabled source's picture wins, else the player's own cape. With Style Smooth, the vanilla cape layer is skipped for that player and the cloth is drawn in its place. With an elytra on, there is no cloth: the winning picture (OptiFine's included) goes on the wings, as in vanilla. This is on by default; turned off, the elytra is plain.
- OptiFine capes, including HD ones (92x44, 184x88), fit the vanilla cape box exactly.
- Public cape sources are tried in the order you set, and each one can be turned on or off.
  - **Animated capes** plays stacked sprite sheets and GIFs. It is on by default.
  - Official Mojang capes stay when no enabled source has a picture.
- Pictures are cached, so a player's cape is downloaded once.


## Look and settings

SuperCapes uses the shared Sunrise dark look: a `#0F1118` card with a 2 px accent edge, tabs and
charcoal chips. The accent is Sunrise orange `#F9801D` at first; change it with **Accent colour**
in the settings card (eight presets, or Reset).
The cape card itself (Pause menu > SuperCapes, or `/supercapes`) uses the same look.

The settings card opens from SunnyMods' **Settings** button, from Sunrise's own Mods list, or
from the mod's settings key. **Key binds › Rebind** switches to Sunrise's classic page to change keys.

## Install (local)

You need the Sunrise launcher with native mods allowed. Use either way below.

- **From the file:** copy `supercapes-1.0.0.smod` into the instance's `sunrise-native-mods/` folder, next to `options.txt`. Turn it on in the launcher's **Mods** page, or add the line `mod.supercapes:true` to `sunrise-mods.txt`. The launcher then compiles it for that instance (`sunrise-launcher native-build`).
- **From GitHub:** once the repository is public, use **Add from GitHub** on the launcher's Mods page, or run `sunrise-launcher mods add-github https://github.com/Zekrenlight/supercapes --trust-native-code`. A release with a `.smod` asset installs that file. Without one, the launcher packs the source at the chosen commit.

Native mods run with full access to your computer. Read the source before you install one.

## Commands

- `/supercapes` or `/supercapes panel`: open or close the card. If the game is paused, the card shows over the pause menu.
- `/supercapes help`

## Files

These live in `sunrise-mods/.sunrise-mod-data/supercapes/` under the game folder:

- `supercapes.cfg`: the settings, format 3. Older files are moved over: Cloth off or Motion Follow becomes Style Blocky, anything else Smooth.
- `cache/`: decoded cape pictures, one file per player UUID. Delete the folder to download them again.


## How it fits into Sunrise

- **Typed mod API:** the card (`screen_render_post` and the screen input hooks), `/supercapes` (`command`), and the cloth (`render_player_post` with `BlockEntityDrawContext::quad`).
- **Native route:** the API has no way to put a downloaded picture into the atlas or onto a player's cape. It also has no way to hide the vanilla cape layer or add a pause-menu button. For these, SuperCapes uses one support module (`src/native/game.rs`, placed beside `native_hooks/render_frame_pre.rs`) and these mixins:
  - a `HEAD` mixin on `Game::native_render_frame_pre`
  - a cancellable `HEAD` mixin on `Game::player_cape_geometry`, which skips the vanilla cape for players the cloth draws (or, with Style Blocky, draws the winning picture)
  - a cancellable `HEAD` mixin on `Game::player_elytra_geometry`, which puts the winning picture on the wings, or no cape (a plain elytra) when Elytra uses cape is off
  - a `BEFORE_CALL`/`AFTER_CALL` pair in `Game::draw_player_preview`, so the inventory preview keeps its cape
  - a `RETURN` mixin on `pause_buttons` and a `HEAD` mixin on `Game::execute_pause_action`, for the pause button

  Once a frame, the support module does four things:
  - read and write the settings
  - start downloads and decode PNGs with Sunrise's own decoder
  - store each frame in a skin slot with `Game::store_skin`
  - set each player's `mob.cape`

  A mod with a mixin works only on the exact build it was made for.


## Building

```sh
cargo sunrise-mod           # fetches the pinned SDK into .sunrise/ (gitignored)
cargo sunrise-mod check     # type-checks against the pinned SDK
cargo test --lib            # unit tests, no game needed
cargo sunrise-mod build     # target/sunrise-mod/supercapes.smod
```

To move to a newer SDK, run `cargo sunrise-mod update --sunrise <build>` (it rewrites the
`sunrise` and `mixins` lines in `sunrise-mod.toml`), then check and rebuild.

Install the `.smod` with sunrise-launcher (**Add from file**), or point **Add from GitHub** at
this repository. Native mods run with full access to your computer; read the source first.

## Compatibility

| Piece | Pin |
| --- | --- |
| Minecraft Java | 26.3 |
| Sunrise API | 0.2 |
| Mod SDK (`[native].sunrise`) | `94256510f74e6843754e0a93fd9e4da7e45b1056` (`9425651`, 19 Nov 2026) |
| Mixins (`[native].mixins`) | `e59da9094f0c2fdda6c627fe9eff22dec6a906f0` |

SuperCapes works on its own and alongside the other Sunrise mods by Zekrenlight. Its settings-card
hooks act only on its own Options page, so several mods can be installed together without any
conflicting replacements. A mod with native hooks works only on the exact SDK build it was made for.

## Building

```sh
cargo sunrise-mod           # fetches the pinned SDK into .sunrise/ (gitignored)
cargo sunrise-mod check     # type-checks against the pinned SDK
cargo test --lib            # unit tests, no game needed
cargo sunrise-mod build     # target/sunrise-mod/supercapes.smod
```

To move to a newer SDK, run `cargo sunrise-mod update --sunrise <build>` (it rewrites the
`sunrise` and `mixins` lines in `sunrise-mod.toml`), then check and rebuild.

Install the `.smod` with sunrise-launcher (**Add from file**), or point **Add from GitHub** at
this repository. Native mods run with full access to your computer; read the source first.

## License

MIT © 2026 Zekrenlight. See [LICENSE](LICENSE).
