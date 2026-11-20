# ElonCut operation manual

Match editor tabs to MCP tools, then read, edit and verify the timeline.

Call `guide_read` without arguments for the topic list. Pass `topic` to read
only the pages relevant to the requested edit.

| Editor tab | Agent tools | Topic |
| --- | --- | --- |
| Text & Captions (文本与字幕) | `title_add`, `title_style_words`, `captions_add` | titles-captions |
| Stickers (贴纸) | `edit_apply` with `addShapeClip` for shapes | stickers-shapes |
| Transitions (转场) | `effects_list`, `clip_update` for a clip's incoming cut, or `edit_apply` with `addTransitionClip` for a separate layer | effects-filters-transitions |
| Effects (效果) | `effects_list`, `clip_update`, `effect_layer_add` | effects-filters-transitions |
| Filters (滤镜) | `effects_list`, `clip_update`, `effect_layer_add` | effects-filters-transitions |
| Adjust (调节) | `effects_list`, `effect_layer_add` with `concat.adjust` | effects-filters-transitions |
| Templates (模板) | `template_list`, `template_instantiate` | templates-export |

For animation and masking, read keyframes-masks. For a vertical short from
local clips, read workflow-vertical-montage. For checks and delivery, read
workflow-verify and templates-export as needed.

1. Before editing, read the target project's active timeline with `timeline_read`.
   If the project is closed, open the agreed project first. Use returned track,
   clip and media ids, and confirm the intended timeline and time range.
2. Query `effects_list` for effect, filter and transition ids and parameter
   ranges. Use the returned ids; never invent ids or infer them from names.
3. When several edits should succeed together and undo together, call
   `edit_apply` with `path` and `command: {"op":"batch","commands":[...]}`.
   Every entry is an editor command object with its own `op`. The batch is
   atomic and `createdId` is only the last id minted. If later edits need a new
   id you do not know yet, create it first and read it back before proceeding.
4. Read back the edited timeline and check the affected picture with
   `preview_frame` at representative times. Leave `output` unset to get
   an inline image. A still cannot prove motion, pacing or audio; report
   which checks you actually made. Save or export when requested.

## What you cannot do

The MCP tools do not provide automatic transcription, speech generation, vocal
isolation, image quality enhancement, a sound library, asset library organization,
or invoking editor preset cards by name. Tell the user which capability is
unavailable. Ask for local media, genuine caption text or timings, or explicit
style settings as appropriate. Never fabricate inputs or claim work you did not do.
