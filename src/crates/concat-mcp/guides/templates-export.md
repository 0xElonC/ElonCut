# Templates and export

Create a project from a template with local slot files, or render and track an export job.

## Make an editable project from a template

1. Call `template_list` without arguments. Use the chosen template's bundle
   folder and read the required slot media ids. Ask which local file belongs
   in a slot if the mapping is unclear.
2. Call `template_instantiate` with `template` (bundle folder), `location`
   (existing parent directory), `name` (new project name) and `fills`, an
   array of `{mediaId, file}`. Fill every slot with a readable absolute local
   file path, not a URL. Files are probed before the new project is made;
   missing or invalid inputs can prevent creation.
3. It opens the new project and returns its summary. Confirm the timeline
   and filled shots with `timeline_read` and `preview_frame` before delivery.

## Render an MP4

1. Open the agreed project and confirm the export destination. Call
   `export_start` with `path` and `output` (an absolute .mp4 path). It returns
   a `job`, not a completed file. Only one export runs at a time; if busy,
   report it rather than cancelling someone else's job.
2. Preserve timeline dimensions and rate unless the user requests overrides.
   Optional fields include `width`/`height`, `rateNum`/`rateDen`, `codec`
   (`h264`, `hevc`, `av1`), `crf` (lower means higher quality and typically
   larger files; default 20) and `preset` (x264 speed preset, default `medium`).
3. Pass `job` to `export_status` for an immediate check or `export_wait` to
   wait with progress. `timeoutSeconds` is 0–600, default 60. A wait returning
   `running` has timed out without stopping the job; check again. If asked to
   stop that job, call `export_cancel` and confirm `cancelled` in its status.
4. Report the output path when state is `done`. For `failed`, report the
   returned error. Export status does not establish that the file meets visual
   or audio expectations. Use workflow-verify for the timeline checks.

## What you cannot do

The save-template API is not exposed through MCP. Do not promise to package
the project as a reusable template; offer to save the editable project if
appropriate. Template slots cannot fetch assets from URLs or a stock, sound,
or asset library. Ask for local files. See the index for other unavailable
features.
