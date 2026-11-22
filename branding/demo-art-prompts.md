# Demo art prompts

Pictures for the browser demo's made-up games. Make them with any image AI, then save them in
`public/demo-art/` with the names below (`.webp`, `.jpg` or `.png`). The demo uses a picture when
it's there and its own drawing when it isn't. Keep each under about 400 KB (webp is smallest),
since the folder ships inside the app.

Keep them original: no real game's logo, characters, or cover layout.

## Sizes

| Kind | Name ending | Size | Notes |
|---|---|---|---|
| Cover | `-cover` | 600 × 900 (2:3, portrait) | The game's title at the top |
| Banner | `-hero` | 1920 × 620 (wide) | No text; the app puts the logo over the left side |
| Logo | `-logo` | 800 × 260, transparent PNG | Just the title, styled |

## Cyberhunk 2088 (`cyberhunk2088-…`)

**Cover:** Original video game cover art, portrait 2:3. A confident young man, chest-up, three-quarter view, undercut hair swept to one side, a sleek wraparound visor glowing cyan to magenta over his eyes, a black high-collar techwear jacket with thin neon seams. Behind him a dense night megacity with glowing windows and holographic signs. Hot yellow background panel with glitchy cyan and magenta bars. Bold title text "CYBERHUNK 2088" at the top in a sharp futuristic font. Stylized semi-realistic digital painting, high contrast, clean composition.

**Banner:** Wide cinematic banner, 1920×620, no text. The same man standing on a rooftop on the right third, looking out over a neon night megacity, rain, holographic billboards, yellow and cyan glow, empty dark space on the left for a logo.

**Logo:** Logo text "CYBERHUNK 2088" on a transparent background, chunky angular futuristic letters, yellow with a cyan glitch offset and a thin magenta outline, no other elements.

## Frostrim: The Elder Moods (`frostrim-…`)

**Cover:** Original fantasy game cover art, portrait 2:3. A broad-shouldered warrior seen from the chest up, steel helmet with two curved ivory horns, thick fur cloak over chainmail, braided red beard, calm grumpy expression, light snow falling. Behind him jagged snowy mountains under a pale blue sky. Title text "FROSTRIM" with a smaller subtitle "The Elder Moods" at the top in carved stone letters. Painterly, cold blue and grey palette with warm skin tones.

**Banner:** Wide cinematic banner, 1920×620, no text. A lone horned warrior on a ridge on the right third, wind blowing his cloak, a vast valley of snowy peaks and pine forests, soft morning light, open sky on the left for a logo.

**Logo:** Logo text "FROSTRIM" with "The Elder Moods" underneath, on a transparent background, weathered silver letters like carved stone with frost on the edges.

## Stardude Valley (`stardudevalley-…`)

**Cover:** Original cozy farming game cover art, portrait 2:3, pixel-art-inspired painterly style. A cheerful young farmer, chest-up, woven straw hat with a red band, freckles, rosy cheeks, red plaid shirt under blue denim overalls, a hoe resting on his shoulder. Behind him rolling green fields, a small wooden barn, a big warm sun and fluffy clouds. Rounded friendly title text "STARDUDE VALLEY" at the top. Bright, warm, inviting colors.

**Banner:** Wide banner, 1920×620, no text. A sunny farm on the right two-thirds: crop rows, a red barn, chickens, the farmer waving; soft sky on the left for a logo.

**Logo:** Logo text "Stardude Valley" on a transparent background, rounded bubbly wooden-sign letters in warm yellow and brown with a little leaf on the "V".

## Free places to make them

- **Microsoft Designer / Bing Image Creator** (free with a Microsoft account): paste a prompt, pick the closest result, download.
- **Ideogram** (free tier): good at putting text like titles into images.
- **Leonardo.ai** (free daily credits): lets you set the exact size.

Then convert to webp if you like (`cwebp -q 80 in.png -o cyberhunk2088-cover.webp`) and drop them in
`public/demo-art/`. Reload the demo to see them.
