// SPDX-License-Identifier: AGPL-3.0-or-later

//! The queue: pieces wait for a turn, a few run at once on worker threads,
//! and what happens to each comes back as events for the window to apply.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::model::{Kind, Phase, Slot, Story, Take};
use crate::prompt::Request;
use crate::store;

/// What a service sends back.
#[derive(Debug, Clone, PartialEq)]
pub struct Made {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// Its extension, without the dot: "png", "mp4", "mp3".
    pub extension: String,
    /// For audio and video, how long it runs.
    pub seconds: Option<f32>,
}

/// A cloud service, or whatever stands in for one. One call makes one piece
/// and may take minutes; `progress` is told how far along it is, 0 to 1,
/// when the service says.
pub trait Backend: Send + Sync {
    /// Its name, kept on what it makes.
    fn name(&self) -> String;
    /// Makes what `request` asks for.
    fn make(&self, request: &Request, progress: &dyn Fn(f32)) -> Result<Made, String>;
}

/// Which piece a job fills: a shot or character, and which of its slots.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Target {
    /// The shot's or the character's id.
    pub id: String,
    /// Which slot.
    pub kind: Kind,
}

/// What happened to a job.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A worker took it.
    Started(Target),
    /// The service reported how far along it is.
    Progress(Target, f32),
    /// It is made and written.
    Done(Target, Take),
    /// It was not made.
    Failed(Target, String),
}

struct Job {
    target: Target,
    request: Request,
}

/// The queue and its workers.
pub struct Queue {
    send: Option<Sender<Job>>,
    events: Receiver<Event>,
    withdrawn: Arc<Mutex<HashSet<Target>>>,
    waiting: Arc<Mutex<HashSet<Target>>>,
}

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn kind_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Portrait => "portrait",
        Kind::Frame => "frame",
        Kind::Video => "video",
        Kind::Voice => "voice",
    }
}

impl Queue {
    /// A queue that makes up to `workers` pieces at once with `backend`,
    /// writing into `project`'s story folder.
    pub fn start(backend: Arc<dyn Backend>, project: PathBuf, workers: usize) -> Queue {
        let (send, jobs) = mpsc::channel::<Job>();
        let (tell, events) = mpsc::channel::<Event>();
        let jobs = Arc::new(Mutex::new(jobs));
        let withdrawn = Arc::new(Mutex::new(HashSet::new()));
        let waiting = Arc::new(Mutex::new(HashSet::new()));
        for _ in 0..workers.max(1) {
            let (jobs, tell, backend, project) =
                (jobs.clone(), tell.clone(), backend.clone(), project.clone());
            let (withdrawn, waiting) = (withdrawn.clone(), waiting.clone());
            std::thread::spawn(move || {
                loop {
                    // The lock is held only while waiting for a job, so the
                    // other workers take the next ones.
                    let job = match jobs.lock().map(|rx| rx.recv()) {
                        Ok(Ok(job)) => job,
                        _ => return,
                    };
                    waiting.lock().unwrap().remove(&job.target);
                    if withdrawn.lock().unwrap().remove(&job.target) {
                        continue;
                    }
                    let _ = tell.send(Event::Started(job.target.clone()));
                    let target = job.target.clone();
                    let report = tell.clone();
                    let progress = move |fraction: f32| {
                        let _ = report.send(Event::Progress(target.clone(), fraction.clamp(0.0, 1.0)));
                    };
                    let outcome = backend
                        .make(&job.request, &progress)
                        .and_then(|made| write(&project, &job.target, &made, &backend.name()));
                    let _ = tell.send(match outcome {
                        Ok(take) => Event::Done(job.target, take),
                        Err(reason) => Event::Failed(job.target, reason),
                    });
                }
            });
        }
        Queue { send: Some(send), events, withdrawn, waiting }
    }

    /// Asks for a piece. Asking again for one already waiting does nothing.
    pub fn submit(&self, target: Target, request: Request) {
        if !self.waiting.lock().unwrap().insert(target.clone()) {
            return;
        }
        self.withdrawn.lock().unwrap().remove(&target);
        if let Some(send) = &self.send {
            let _ = send.send(Job { target, request });
        }
    }

    /// Takes a piece out of the line if it has not begun. One that is
    /// already running finishes.
    pub fn withdraw(&self, target: &Target) {
        if self.waiting.lock().unwrap().contains(target) {
            self.withdrawn.lock().unwrap().insert(target.clone());
        }
    }

    /// What has happened since the last call.
    pub fn events(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        // Workers end when the sending side is gone.
        self.send = None;
    }
}

fn write(project: &std::path::Path, target: &Target, made: &Made, service: &str) -> Result<Take, String> {
    let folder = store::assets(project);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let name = format!(
        "{}-{}-{}.{}",
        target.id,
        kind_word(target.kind),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        made.extension
    );
    let path = folder.join(&name);
    std::fs::write(&path, &made.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Take {
        path: format!("story/{name}"),
        service: service.to_owned(),
        made: now(),
        seconds: made.seconds,
    })
}

fn slot_mut<'a>(story: &'a mut Story, target: &Target) -> Option<&'a mut Slot> {
    match target.kind {
        Kind::Portrait => story
            .cast
            .iter_mut()
            .find(|c| c.id == target.id)
            .map(|c| &mut c.portrait),
        Kind::Frame => story.shot_mut(&target.id).map(|s| &mut s.frame),
        Kind::Video => story.shot_mut(&target.id).map(|s| &mut s.video),
        Kind::Voice => story.shot_mut(&target.id).map(|s| &mut s.voice),
    }
}

/// Marks a piece as waiting in the queue.
pub fn mark_queued(story: &mut Story, target: &Target) {
    if let Some(slot) = slot_mut(story, target) {
        slot.phase = Phase::Queued;
    }
}

/// Puts what happened in the story. Returns false when the piece is gone,
/// the shot having been deleted while it was made.
pub fn apply(story: &mut Story, event: &Event) -> bool {
    let target = match event {
        Event::Started(t) | Event::Progress(t, _) | Event::Done(t, _) | Event::Failed(t, _) => t,
    };
    let Some(slot) = slot_mut(story, target) else {
        return false;
    };
    match event {
        Event::Started(_) => slot.phase = Phase::Running { progress: 0.0 },
        Event::Progress(_, progress) => slot.phase = Phase::Running { progress: *progress },
        Event::Done(_, take) => slot.add(take.clone()),
        Event::Failed(_, reason) => slot.phase = Phase::Failed { reason: reason.clone() },
    }
    true
}

/// A stand-in for the cloud services, which makes a plain file of each kind
/// after a short wait: for trying the window without keys or money, and for
/// tests.
pub struct Rehearsal {
    /// How long each piece takes.
    pub delay: std::time::Duration,
}

impl Backend for Rehearsal {
    fn name(&self) -> String {
        "rehearsal".into()
    }

    fn make(&self, request: &Request, progress: &dyn Fn(f32)) -> Result<Made, String> {
        for step in 1..=4 {
            std::thread::sleep(self.delay / 4);
            progress(step as f32 / 4.0);
        }
        let (width, height) = request.frame;
        match request.kind {
            Kind::Portrait | Kind::Frame => {
                let hue = request.prompt.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)) % 360;
                let svg = format!(
                    "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='{height}' viewBox='0 0 {width} {height}'>\
                     <defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'>\
                     <stop offset='0' stop-color='hsl({hue},45%,38%)'/><stop offset='1' stop-color='hsl({},30%,10%)'/>\
                     </linearGradient></defs><rect width='100%' height='100%' fill='url(#g)'/></svg>",
                    (hue + 40) % 360
                );
                Ok(Made { bytes: svg.into_bytes(), extension: "svg".into(), seconds: None })
            }
            Kind::Video => Ok(Made {
                bytes: b"rehearsal video".to_vec(),
                extension: "txt".into(),
                seconds: Some(request.seconds),
            }),
            Kind::Voice => Ok(Made {
                bytes: b"rehearsal voice".to_vec(),
                extension: "txt".into(),
                seconds: Some((request.speech.chars().count() as f32 * 0.18).max(0.8)),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::shot_request;
    use std::time::{Duration, Instant};

    fn wait_for(queue: &Queue, want: usize) -> Vec<Event> {
        let mut seen = Vec::new();
        let start = Instant::now();
        while seen.iter().filter(|e| matches!(e, Event::Done(..) | Event::Failed(..))).count() < want {
            assert!(start.elapsed() < Duration::from_secs(10), "timed out: {seen:?}");
            seen.extend(queue.events());
            std::thread::sleep(Duration::from_millis(5));
        }
        seen
    }

    fn story() -> (Story, String) {
        let mut story = Story::new("t");
        story.add_character("林夏", "x", "short hair");
        let s = story.add_scene("shop");
        let h = story.add_shot(&s, "stares at a watch", 4.0).unwrap();
        (story, h)
    }

    #[test]
    fn a_frame_is_made_written_and_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let (mut story, h) = story();
        let queue = Queue::start(Arc::new(Rehearsal { delay: Duration::from_millis(8) }), dir.path().into(), 2);
        let target = Target { id: h.clone(), kind: Kind::Frame };
        let request = shot_request(&story, &h, Kind::Frame, dir.path()).unwrap();
        mark_queued(&mut story, &target);
        assert_eq!(story.shot(&h).unwrap().1.frame.phase, Phase::Queued);
        queue.submit(target, request);

        for event in wait_for(&queue, 1) {
            assert!(apply(&mut story, &event));
        }
        let frame = &story.shot(&h).unwrap().1.frame;
        assert_eq!(frame.phase, Phase::Done);
        let take = frame.current().unwrap();
        assert!(dir.path().join(&take.path).is_file());
        assert_eq!(take.service, "rehearsal");
    }

    #[test]
    fn asking_twice_for_a_piece_that_is_waiting_makes_it_once() {
        let dir = tempfile::tempdir().unwrap();
        let (story, h) = story();
        let queue = Queue::start(Arc::new(Rehearsal { delay: Duration::from_millis(40) }), dir.path().into(), 1);
        let target = Target { id: h.clone(), kind: Kind::Frame };
        let request = shot_request(&story, &h, Kind::Frame, dir.path()).unwrap();
        queue.submit(target.clone(), request.clone());
        queue.submit(target, request);
        let events = wait_for(&queue, 1);
        std::thread::sleep(Duration::from_millis(120));
        let more = queue.events();
        let dones = events.iter().chain(more.iter()).filter(|e| matches!(e, Event::Done(..))).count();
        assert_eq!(dones, 1);
    }

    #[test]
    fn a_piece_withdrawn_before_it_starts_is_never_made() {
        let dir = tempfile::tempdir().unwrap();
        let (mut story, h) = story();
        let s = story.scenes[0].id.clone();
        let h2 = story.add_shot(&s, "second", 3.0).unwrap();
        // One worker, busy with the first piece while the second waits.
        let queue = Queue::start(Arc::new(Rehearsal { delay: Duration::from_millis(80) }), dir.path().into(), 1);
        let first = Target { id: h.clone(), kind: Kind::Frame };
        let second = Target { id: h2.clone(), kind: Kind::Frame };
        queue.submit(first, shot_request(&story, &h, Kind::Frame, dir.path()).unwrap());
        queue.submit(second.clone(), shot_request(&story, &h2, Kind::Frame, dir.path()).unwrap());
        queue.withdraw(&second);
        let events = wait_for(&queue, 1);
        std::thread::sleep(Duration::from_millis(200));
        let all: Vec<_> = events.into_iter().chain(queue.events()).collect();
        assert!(all.iter().all(|e| !matches!(e, Event::Done(t, _) | Event::Started(t) if *t == second)));
    }

    #[test]
    fn a_failure_is_kept_as_the_reason_and_a_deleted_shot_is_noticed() {
        let (mut story, h) = story();
        let target = Target { id: h.clone(), kind: Kind::Video };
        assert!(apply(&mut story, &Event::Failed(target.clone(), "refused".into())));
        assert_eq!(
            story.shot(&h).unwrap().1.video.phase,
            Phase::Failed { reason: "refused".into() }
        );
        story.scenes[0].shots.clear();
        assert!(!apply(&mut story, &Event::Started(target)));
    }
}
