# SuperCapes 1.0.0 specification

SuperCapes is an original Sunrise native mod (`.smod`, MIT). It is written in Rust. It is not a port of a Java or Fabric cape mod, and it does not share code, assets, or config keys with one.

## Settings

Stored in `supercapes.cfg` under `sunrise-mods/.sunrise-mod-data/supercapes/`, format version 3. A format 1 or 2 file is moved over: `cloth=no` or `motion=follow` becomes `style=blocky`, anything else `style=smooth`. Those two keys are no longer written. A format 1 file also gets the default ripple, quality, weight and ripple strength.

- `online` (default yes): master switch for remote cape lookups.
- `only_self` (default no).
- `elytra` (default yes): leave the resolved cape on `mob.cape` while an elytra is worn so vanilla wings can use it; off draws the wings with no cape (the plain elytra texture), never the game's own cape. The cloth is never drawn while an elytra is worn.
- `animated` (default yes): cycle sprite-sheet and GIF frames; off keeps the first frame.
- `style` smooth | blocky (default smooth): smooth is the segmented cloth below; blocky leaves the cape to the vanilla cape layer (rigid, vanilla physics) with the winning picture. Ripple, quality, weight and ripple strength apply to smooth only; the card hides them for blocky.
- `wind` off | waves (default waves): the ripple, a small wave along the cape only while the player moves (never at rest).
- `quality` low | medium | high (default medium): 8 / 16 / 24 segments.
- `gravity` (weight) 0–9, default 5: scales the swing from 1.0 (vanilla) down by 0.07 a step. `wind_strength` (ripple strength) 0–9, default 4: up to 18° at full speed with 9.
- `order` and `source.<id>`: OptiFine and MinecraftCapes default on; LabyMod and Cloaks default off.

## Sources

Lookups are first enabled source in user order. Only a public picture (or a JSON body that names one, for MinecraftCapes) is requested, with User-Agent `SuperCapes/1.0.0`. Workers download bytes; the game thread decodes PNG or GIF, slices a tall 2:1 sheet when it has three or more frames (or when MinecraftCapes sets `animatedCape`), plays GIF frame delays (or a profile `frameDelay`, else 100 ms), reduces HD OptiFine pictures (a multiple of 46x22 larger than 46x22) to 46x22 by averaging each block, crops each frame with Sunrise's cape slot helper (`skins::process_cape`, which keeps a picture up to 64x32 1:1 in the vanilla layout), stores every frame in the atlas once, and writes `{uuid}.bin` (format SC3) under `cache/`. Animation swaps the current `capes/…-fN` sprite on `mob.cape` (elytra) and on the cloth quads; frames are not re-uploaded each tick.

## Cloth

A segmented cape (Low 8, Medium 16, High 24 segments), sized as the vanilla cape box (10x16x1 model pixels at the player model's 15/16 scale). Its top sits at the collar and its middle 2.5 model pixels behind the body.

The drive is the vanilla cape's: a cloak point trails the player, closing a quarter of the gap each tick. The gap gives the collar targets:
- forward lean: `clamp(forward*100, 0, 150)/2`
- vertical flap: `clamp(dy*10, -6, 32)`
- sideways sway: `clamp(side*100, -20, 20)/2`, plus turning speed

Crouching adds 25°. The targets are scaled by weight, clamped to 70° out and 12° to the side, and eased in at 1/tick. Each segment then follows the one above it at 2.6/tick, plus a small extra bend (35% of the swing over the length), so changes travel down with a delay.

While moving, a travelling sine runs down the segments:
- amplitude: up to 18° times strength/9, times speed, growing toward the hem
- speed: about one wave every 8 ticks at a sprint

At rest every angle snaps to 0 and the cape hangs straight down.

The mesh is a closed box bent along the segments:
- per segment, an outer face (front texels 1..11, the segment's slice of 1..17), an inner face (back 12..22, mirrored) and two edge faces (0..1, 11..12)
- top (1..11 x 0..1) and bottom (11..21 x 0..1) faces

Rows share vertices, and each face is wound to face outward.

For every player the cloth draws, the vanilla cape layer (`player_cape_geometry`) is skipped. It is not skipped while an elytra is worn, in the inventory preview, or while the player is sleeping, swimming, crawling, or using riptide: the cloth is drawn in a standing frame and would hang through the head, so those poses keep vanilla's cape, which follows the laid-over body. When the game is about to draw a cape picture other than the winning one, the cape layer and `player_elytra_geometry` draw again with the winning picture. Elytra wings use the picture's elytra area (22..46 x 0..22 of the 64x32 layout).

## UI

Hookline-style card drawn in `screen_render_post`. Text is measured with the game font: About lines word-wrap to the card's inner width, chip, tab and source labels are cut with "..." to fit. Pause menu uses an overlay (a mod screen does not open over Pause). Close is applied in `render_frame_pre`. No keybind.

The pause button is added to the game's own pause layout (`pause_buttons`) as a full-width row (action 37, label `SuperCapes`) at Save and Quit's place, and Save and Quit moves down one row. The game draws it with its button sprites. The rows above (server links, other mods' pages) and the icon row keep their places.

## Network and privacy

No author API, news, credits, or PSA. No machine or hardware identifiers. No cookies, account tokens, or telemetry. Official Mojang capes remain when no source returns a picture. Turning Online capes off, Only me on, or a source off hides that source's picture the same frame, including one already fetched, cached, or left on the player, and puts the official cape back. The on-disk cache is kept so turning the source back on does not need another download.
