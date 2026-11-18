// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Live mode: the window's open project, as the API sees it.
//!
//! An agent's calls arrive on the API's thread; the project is the window's,
//! on the event loop's. This hands each call across: a read is of what is on
//! screen at that moment, and an edit is applied by the window itself - the
//! same `Studio::apply` a click reaches - so it is undone with the window's
//! Undo, joins its history, and is drawn by the panes that draw every edit.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use concat_api::Live;
use concat_host::Session;
use concat_host::session::EditorView;
use concat_project::Command;

use crate::studio::Studio;

/// How long a call waits for the window before it gives up: a window that
/// is busy for longer than this, or closing, is not going to answer.
const PATIENCE: Duration = Duration::from_secs(15);

/// The window, reachable from the API's thread.
pub struct WindowLive {
    /// Off while the person has live mode turned off: the window's project
    /// is then no one's but theirs.
    pub on: Arc<AtomicBool>,
}

/// Runs `body` on the event-loop thread with the studio and waits for what
/// it returns.
fn ask<R: Send + 'static>(
    body: impl FnOnce(&mut Studio) -> R + Send + 'static,
) -> Result<R, String> {
    let (send, receive) = mpsc::channel();
    crate::host::on_ui(move |studio, _, _| {
        let _ = send.send(body(studio));
    });
    receive
        .recv_timeout(PATIENCE)
        .map_err(|_| "The window did not answer: it is busy or closing".to_owned())
}

/// The state of the project the window has open, as the API reports it.
fn view_of(studio: &Studio, created: Option<String>) -> Result<EditorView, String> {
    let session = studio.session.as_ref().ok_or("No project is open")?;
    let mut view = session.view();
    view.created_id = created;
    Ok(view)
}

impl Live for WindowLive {
    fn path(&self) -> Option<String> {
        if !self.on.load(Ordering::Relaxed) {
            return None;
        }
        ask(|studio| studio.session.as_ref().map(|session| session.path().to_owned()))
            .ok()
            .flatten()
    }

    fn snapshot(&self) -> Result<Session, String> {
        let (path, settings, document) = ask(|studio| {
            studio
                .session
                .as_ref()
                .map(|session| (session.path().to_owned(), session.settings(), session.document()))
        })?
        .ok_or("No project is open")?;
        Session::detached(&path, settings, &document)
    }

    fn apply(&self, command: Command) -> Result<EditorView, String> {
        ask(move |studio| {
            let session = studio.session.as_ref().ok_or("No project is open")?;
            // Tried on a copy first: the window reports a refused edit to
            // the person as a notice and says nothing to the caller, and an
            // agent needs the reason.
            let mut trial =
                Session::detached(session.path(), session.settings(), &session.document())?;
            trial.apply(command.clone())?;
            let created = studio.apply(command);
            view_of(studio, created)
        })?
    }

    fn undo(&self) -> Result<EditorView, String> {
        ask(|studio| {
            studio.undo();
            view_of(studio, None)
        })?
    }

    fn redo(&self) -> Result<EditorView, String> {
        ask(|studio| {
            studio.redo();
            view_of(studio, None)
        })?
    }

    fn save(&self) -> Result<(), String> {
        ask(|studio| {
            if studio.session.is_none() {
                return Err("No project is open".to_owned());
            }
            studio.save(true);
            Ok(())
        })?
    }
}
