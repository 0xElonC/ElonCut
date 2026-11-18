// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! The agent behind the window's chat.
//!
//! The agent is the Codex installed on this computer, run as its
//! `app-server`: the same models, reasoning depths, sign-in and settings
//! the person already has, with nothing copied into the app. This crate
//! starts it and speaks its protocol (JSON-RPC lines over standard input and
//! output), and says what comes back in a small vocabulary the window can
//! draw without knowing Codex's.
//!
//! - [`locate`] finds the `codex` program.
//! - [`rpc`] is the process and the request/response plumbing.
//! - [`wire`] builds the requests this app makes and reads what Codex says
//!   back ([`wire::Update`]).
//! - [`store`] is the app's own list of its conversations.
//!
//! Nothing here knows about the window, or about video: the agent reaches
//! the project through the Concat MCP server it is started with
//! (see [`wire::ThreadSetup`]), the same tools any agent has.

// The wire and store types are plain records named for what Codex calls
// them; their fields say what they are, and each type says what it is for.
#![allow(missing_docs)]

pub mod locate;
pub mod rpc;
pub mod store;
pub mod wire;
