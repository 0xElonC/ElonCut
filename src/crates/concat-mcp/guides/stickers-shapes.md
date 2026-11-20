# Stickers and shapes

Place an editable geometric overlay using an explicit shape style.

The Stickers tab currently provides shapes. To place one, call `edit_apply`
with the project `path` and an `addShapeClip` command. For example:

```json
{
  "path": "/absolute/path/to/project",
  "command": {
    "op": "addShapeClip",
    "above": true,
    "start": 2,
    "duration": 3,
    "name": "Yellow callout",
    "style": {"kind": "square", "width": 0.35, "height": 0.14, "cornerRadius": 0.3, "fill": "#ffd400"}
  }
}
```

- `start` is timeline seconds (floored at 0); `duration` defaults to 4 s
  and is floored at 1/60 s. Set `trackId` to a known track, or omit it with
  `above: true` to pick a lane above the picture. Without `above`, the engine
  chooses a free lane. Read back actual placement; an occupied lane can shift it.
- `name` labels the clip, not a preset. Omit `style` for the default shape.
  `style.kind` accepts `square` (also used for rectangles), `circle` (also
  ellipses), `triangle`, `parallelogram`, `trapezoid`, `line`, `arrow`,
  `polygon`, `star` and `heart`.
- `width` (0.01–4), `height` (0–4) and `strokeWidth` (0–0.5) are fractions
  of frame height. A zero height gives the natural proportions; lines and
  arrows use width as length and stroke width as thickness. `fill` and `stroke`
  take hex colors such as `#ffd400` or `#ffd40080`; empty fill gives an outline
  alone. `cornerRadius` is 0–1, `sides` is 3–12 for polygons, `points` is 3–24
  for stars and `innerRadius` is 0.05–0.95 for stars.

Use the returned `createdId` with `clip_transform` to position the shape,
and `preview_frame` to check size, contrast and overlap.

## What you cannot do

There is no sticker search, animated sticker library, or preset-card call by
name. Build a supported shape with explicit settings, or ask the user for a
local image they want imported. Do not claim to have fetched or generated a
sticker. See the index for other unavailable features.
