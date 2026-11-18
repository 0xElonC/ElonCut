// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! The `codex app-server` process and its JSON-RPC lines.
//!
//! One line of JSON each way. A line with an `id` and a `result` or `error`
//! answers a request this side made; one with a `method` and no `id` is a
//! notification; one with a `method` and an `id` is a request from Codex
//! that wants an answer. Codex leaves out the `"jsonrpc"` member, so this
//! does too.
//!
//! Nothing here blocks the caller except [`Codex::call_blocking`], which is
//! for a worker thread: the window asks with [`Codex::call`] and is
//! answered on the reader thread.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use serde_json::{Value, json};

/// What a request came back as: the `result`, or the error's message.
pub type Reply = Result<Value, String>;

/// What Codex says without being asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    /// An event: a thread started, words arrived, a tool finished.
    Notification { method: String, params: Value },
    /// A question that waits for [`Codex::respond`].
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// The process ended, with the last thing it said on its error stream.
    Closed(String),
}

/// One line, read.
#[derive(Debug, PartialEq)]
pub enum Line {
    Response { id: u64, reply: Reply },
    Incoming(Incoming),
}

/// Reads a line of the protocol. `None` for anything that is not one.
pub fn read_line(line: &str) -> Option<Line> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    let object = value.as_object()?;
    let id = object.get("id").cloned();
    let method = object.get("method").and_then(Value::as_str);
    match (method, id) {
        (Some(method), Some(id)) => Some(Line::Incoming(Incoming::Request {
            id,
            method: method.to_owned(),
            params: object.get("params").cloned().unwrap_or(Value::Null),
        })),
        (Some(method), None) => Some(Line::Incoming(Incoming::Notification {
            method: method.to_owned(),
            params: object.get("params").cloned().unwrap_or(Value::Null),
        })),
        (None, Some(id)) => {
            let id = id.as_u64()?;
            let reply = match object.get("error") {
                Some(error) => Err(error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex refused the request")
                    .to_owned()),
                None => Ok(object.get("result").cloned().unwrap_or(Value::Null)),
            };
            Some(Line::Response { id, reply })
        }
        (None, None) => None,
    }
}

type Waiting = Box<dyn FnOnce(Reply) + Send>;

/// A running `codex app-server`.
pub struct Codex {
    stdin: Arc<Mutex<ChildStdin>>,
    next: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, Waiting>>>,
    child: Arc<Mutex<Child>>,
}

impl Codex {
    /// Starts the program as an app-server. `on_incoming` is called on the
    /// reader thread for everything Codex says unasked, and once with
    /// [`Incoming::Closed`] when the process ends for any reason.
    pub fn spawn(
        program: &Path,
        on_incoming: impl Fn(Incoming) + Send + 'static,
    ) -> Result<Codex, String> {
        let mut command = Command::new(program);
        command
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window behind the editor.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("{} could not be started: {error}", program.display()))?;
        let stdin = child.stdin.take().ok_or("Codex has no input")?;
        let stdout = child.stdout.take().ok_or("Codex has no output")?;
        let stderr = child.stderr.take().ok_or("Codex has no error stream")?;

        let pending: Arc<Mutex<HashMap<u64, Waiting>>> = Arc::new(Mutex::new(HashMap::new()));
        let tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

        let keep = Arc::clone(&tail);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                log::debug!("agent: codex says: {line}");
                let mut keep = keep.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                if keep.len() == 12 {
                    keep.pop_front();
                }
                keep.push_back(line);
            }
        });

        let waiting = Arc::clone(&pending);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                match read_line(&line) {
                    Some(Line::Response { id, reply }) => {
                        let waiter = waiting
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .remove(&id);
                        if let Some(waiter) = waiter {
                            waiter(reply);
                        }
                    }
                    Some(Line::Incoming(incoming)) => on_incoming(incoming),
                    None => log::debug!("agent: a line that is not protocol: {line}"),
                }
            }
            // Ended: everything still waiting for an answer gets this one.
            let said = tail
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .cloned()
                .unwrap_or_default();
            let stopped = if said.is_empty() {
                "Codex stopped".to_owned()
            } else {
                format!("Codex stopped: {said}")
            };
            let waiters: Vec<Waiting> = waiting
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .drain()
                .map(|(_, waiter)| waiter)
                .collect();
            for waiter in waiters {
                waiter(Err(stopped.clone()));
            }
            on_incoming(Incoming::Closed(stopped));
        });

        Ok(Codex {
            stdin: Arc::new(Mutex::new(stdin)),
            next: AtomicU64::new(1),
            pending,
            child: Arc::new(Mutex::new(child)),
        })
    }

    fn write(&self, message: &Value) -> Result<(), String> {
        let mut stdin = self
            .stdin
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        writeln!(stdin, "{message}")
            .and_then(|()| stdin.flush())
            .map_err(|error| format!("Codex could not be written to: {error}"))
    }

    /// Asks `method`; `then` is called with the answer on the reader thread,
    /// or at once with the reason the question could not be sent.
    pub fn call(
        &self,
        method: &str,
        params: Value,
        then: impl FnOnce(Reply) + Send + 'static,
    ) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, Box::new(then));
        if let Err(error) = self.write(&json!({ "id": id, "method": method, "params": params })) {
            let waiter = self
                .pending
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&id);
            if let Some(waiter) = waiter {
                waiter(Err(error));
            }
        }
    }

    /// [`Codex::call`], waited for. For a worker thread, never the window's.
    pub fn call_blocking(&self, method: &str, params: Value, patience: Duration) -> Reply {
        let (send, receive) = mpsc::channel();
        self.call(method, params, move |reply| {
            let _ = send.send(reply);
        });
        receive
            .recv_timeout(patience)
            .unwrap_or_else(|_| Err(format!("Codex did not answer {method} in time")))
    }

    /// Says something that wants no answer.
    pub fn notify(&self, method: &str, params: Value) {
        let message = if params.is_null() {
            json!({ "method": method })
        } else {
            json!({ "method": method, "params": params })
        };
        if let Err(error) = self.write(&message) {
            log::warn!("agent: {error}");
        }
    }

    /// Answers a [`Incoming::Request`].
    pub fn respond(&self, id: Value, result: Value) {
        if let Err(error) = self.write(&json!({ "id": id, "result": result })) {
            log::warn!("agent: {error}");
        }
    }

    /// The opening exchange: who is asking, then the word that says the
    /// asking may begin. The reply says where Codex keeps its home.
    pub fn handshake(&self, name: &str, title: &str, version: &str) -> Reply {
        let reply = self.call_blocking(
            "initialize",
            json!({
                "clientInfo": { "name": name, "title": title, "version": version },
                "capabilities": { "experimentalApi": true },
            }),
            Duration::from_secs(20),
        )?;
        self.notify("initialized", Value::Null);
        Ok(reply)
    }

    /// Ends the process.
    pub fn kill(&self) {
        let mut child = self
            .child
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for Codex {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_kinds_of_line_are_told_apart() {
        assert_eq!(
            read_line(r#"{"id":3,"result":{"ok":true}}"#),
            Some(Line::Response {
                id: 3,
                reply: Ok(json!({ "ok": true }))
            })
        );
        assert_eq!(
            read_line(r#"{"id":4,"error":{"code":-32600,"message":"no such thread"}}"#),
            Some(Line::Response {
                id: 4,
                reply: Err("no such thread".into())
            })
        );
        assert_eq!(
            read_line(r#"{"method":"turn/started","params":{"threadId":"t"}}"#),
            Some(Line::Incoming(Incoming::Notification {
                method: "turn/started".into(),
                params: json!({ "threadId": "t" })
            }))
        );
        assert_eq!(
            read_line(r#"{"id":"a","method":"item/tool/requestUserInput","params":{}}"#),
            Some(Line::Incoming(Incoming::Request {
                id: json!("a"),
                method: "item/tool/requestUserInput".into(),
                params: json!({})
            }))
        );
    }

    #[test]
    fn what_is_not_protocol_is_skipped() {
        assert_eq!(read_line("warming up…"), None);
        assert_eq!(read_line("[1,2]"), None);
        assert_eq!(read_line("{}"), None);
    }

    #[test]
    fn a_program_that_is_not_there_says_so() {
        let error = Codex::spawn(Path::new("/no/such/codex"), |_| {})
            .err()
            .expect("cannot start");
        assert!(error.contains("could not be started"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_program_that_ends_answers_what_waits() {
        // `true` exits at once: the question in flight is answered with the
        // stop, and the close is reported.
        let (closed, heard) = mpsc::channel();
        let codex = Codex::spawn(Path::new("/usr/bin/true"), move |incoming| {
            if let Incoming::Closed(_) = incoming {
                let _ = closed.send(());
            }
        })
        .expect("true starts");
        let reply = codex.call_blocking("ping", json!({}), Duration::from_secs(5));
        assert!(reply.is_err());
        heard.recv_timeout(Duration::from_secs(5)).expect("closed");
    }
}
