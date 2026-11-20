# guide-tool rev2 evidence

Task scope: the embedded manual framework and one placeholder index only.
The guide-content task has not been started.

## Implemented behavior and matching evidence

| Acceptance | Implementation and verification |
| --- | --- |
| Read-only tool in tools/list | tools.rs registers guide_read with readOnlyHint=true, destructiveHint=false and idempotentHint=true. tests/e2e.rs::check_embedded_guide asserts those actual MCP response fields. |
| Directory, topic, unknown topic | The same e2e helper omits arguments to get the directory and its index summary, reads topic=index and compares the complete text with include_str!("../guides/index.md"), then verifies isError=true and an available-id list for missing-topic. |
| Compile-time embedding, no skill installation | build.rs scans guides/*.md at build time and generates a registry containing include_str! calls. src/guides.rs includes that registry and reads only static strings. No runtime file reader or writer exists in this module. |
| Both MCP modes, no open project | The existing headless and window e2e tests both call check_embedded_guide before project_create. tools::tests::guide_read_never_calls_the_backend uses a backend that panics on any call or describe invocation; directory, known topic and unknown topic all complete without reaching it. |
| Tool-name check and intentional invalid name | guides::tests::every_embedded_guide_references_existing_tools checks every embedded guide against tools::catalogue. guides::tests::tool_name_check_rejects_an_invented_tool deliberately supplies `nonexistent_tool` and asserts the exact rejection: unknown tool `nonexistent_tool`. |
| Agent instructions and live-mode documentation | agent.rs::instructions asks for the directory before editing, then relevant topics. The new instructions test covers all four combinations of project present/absent and live on/off. docs/MCP.md lists guide_read and describes the implemented Live mode toggle, its default, Undo behavior, and the difference between the built-in bridge and an external Automation connection. |

## Executed checks

Commands ran from src/:

```text
$ cargo test -p concat-mcp
Finished `test` profile [unoptimized + debuginfo] target(s) in 16.76s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo clippy -p concat-mcp
Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.39s
Exit code: 0; no warnings.

$ cargo test -p concat instructions
Finished `test` profile [unoptimized + debuginfo] target(s) in 8m 27s
test agent::tests::instructions_read_the_embedded_guide_before_editing_in_every_mode ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 219 filtered out
Binary tests: 0 passed; 0 failed; 6 filtered out.
Exit code: 0.
```

The 14 library tests include the three guides tests and
tools::tests::guide_read_never_calls_the_backend. The three e2e tests are:

- an_agent_edits_previews_and_exports_a_project_headless
- through_a_running_server_the_agent_shares_its_api
- a_modern_client_discovers_and_is_told_which_versions_are_spoken

The window and headless child MCP processes exited and the temporary window
API server was stopped by the existing e2e teardown.

The concat build emitted existing Slint warnings for viewport-height in
ui/primitives/modal.slint:248 and :252, and the unused TextLink component in
ui/start.slint:1331. They did not prevent compilation or test success; those
UI files are outside this task's change scope.

## Files inspected for runtime disk writes and live-mode behavior

- src/crates/concat-mcp/build.rs: build-time reads of source Markdown; writes
  only generated Rust registration code into Cargo OUT_DIR.
- src/crates/concat-mcp/src/guides.rs: static registry lookup, formatting and
  argument validation only; no filesystem access or skill installation.
- src/crates/concat-mcp/src/tools.rs: GuideRead dispatch stays local and
  returns MCP text; no backend request and no disk-write path.
- src/crates/concat-mcp/src/protocol.rs and src/lib.rs: modes share the same
  Toolbox; protocol responses are written to the MCP stream.
- src/crates/concat-mcp/src/backend.rs: Window forwards API requests to its
  socket, Headless dispatches to the local API; GuideRead uses neither path.
- src/crates/concat/src/agent.rs: only prompt text and a related test changed;
  its existing bridge calls api.set_live(WindowLive), starts with live=true,
  and updates the shared flag from on_live_changed.
- src/crates/concat/src/live.rs: path() respects the flag; apply/undo/redo
  execute through the window's event loop and Studio editing/history methods.
- docs/MCP.md: documents the manual without instructing skill installation.

## Content handoff and limits

Add files named with lowercase topic ids and .md under guides/; the build
automatically includes all of them. Put an H1 heading before a one-line
summary (the first nonblank line after the H1). License/source headers before
the heading are preserved because include_str! embeds the full original file.
Read one topic with {"topic":"topic-id"}; omit arguments to list every topic.

The tool-name check treats standalone snake_case identifiers in single
inline backticks as tool references. Command ops and argument names remain
camelCase; fenced code examples are skipped. This check validates tool names,
not command parameters, effect ids or editing semantics. Unsupported tool
names should be described in plain prose, not formatted as callable tools.

No manual body beyond the placeholder index is delivered in this task.
No real user projects, real media, GUI live-edit session or external MCP
clients were exercised; modes were tested through the existing process e2e
harness with temporary directories and an ephemeral local API server.
Live-mode documentation was checked against source, without changing that
implementation or trying to validate its separate editing feature.

## Source snapshot

Base HEAD: edcdc47bcfa9ab43ad36dcecae7dc102a6c71abb. E-Flow owns the eventual
task checkpoint; this record identifies the tested working-tree contents
without making a separate Git commit. The code reference is
sha256:c864faa72df2ac71054bc7b103b8aac77c9e0e662ab19249f1dbadcb870d5bae.
It is SHA-256 over the UTF-8 manifest below, sorted by path, with two spaces
between each per-file SHA-256 and path and a newline after every row. This
evidence document is excluded from the source manifest.

```text
7b34d8a1ff712a6662f575f76daebc06af406da372a7b3ce07ca8367f682d87b  docs/MCP.md
63eef1f20e3fa80164cbc9cbcefb83f9fae77839e888d0609921a45384e2d4b7  src/crates/concat-mcp/build.rs
37ac24fbca372ef477e80c584703aa324787430574651fcd41f87e447403f29e  src/crates/concat-mcp/guides/index.md
c4fbdf8d80c3a15434fef884ef5c760fea5b01eb5986f4f6f43a8d9e75c264c3  src/crates/concat-mcp/src/guides.rs
1530cb689a1389c9407c594a8c9c26797d4f6e45f421d85e4a4479277fe58e57  src/crates/concat-mcp/src/lib.rs
3733bd10fc65948c20004814bbbd081b2fbb48a3e84f227d4b75a0712a3eb9c7  src/crates/concat-mcp/src/protocol.rs
68858b25c1bdfe64d94524f406e306740705d8a4acbde667858e08e82a1d5d84  src/crates/concat-mcp/src/tools.rs
4fe68262be3ed7c5c26ab3486f3325e7968944f5aeb989b82f1398a1a30ce59b  src/crates/concat-mcp/tests/e2e.rs
275fcd12a905cff96eaa0c3e45fbf735c8c9ba9b78039d6792ba1249f9f81b38  src/crates/concat/src/agent.rs
```
