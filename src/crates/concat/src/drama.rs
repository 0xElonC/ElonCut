// SPDX-License-Identifier: AGPL-3.0-or-later

//! The Story screen's controller.
//!
//! The screen holds nothing of its own. This reads the project's
//! `story.json` when the screen comes up, puts what the person sees on the
//! `StoryState` global, writes the story back after every edit, and runs
//! the queue that makes each shot's pieces: what the queue reports is
//! applied on a short timer, on the event-loop thread.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use concat_drama::{
    Event, Kind, Line, Motion, Phase, Queue, Rehearsal, ShotSize, Story, Target, jobs, prompt, store,
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::host::Shell;
use crate::i18n::{t, tf};
use crate::ui::{App, StoryScene, StoryShot, StoryState};

const SIZES: [(ShotSize, &str); 5] = [
    (ShotSize::Wide, "story.sizeWide"),
    (ShotSize::Full, "story.sizeFull"),
    (ShotSize::Medium, "story.sizeMedium"),
    (ShotSize::Close, "story.sizeClose"),
    (ShotSize::Detail, "story.sizeDetail"),
];

const MOTIONS: [(Motion, &str); 8] = [
    (Motion::Still, "story.motionStill"),
    (Motion::PushIn, "story.motionPushIn"),
    (Motion::PullOut, "story.motionPullOut"),
    (Motion::Pan, "story.motionPan"),
    (Motion::Track, "story.motionTrack"),
    (Motion::Follow, "story.motionFollow"),
    (Motion::Orbit, "story.motionOrbit"),
    (Motion::Handheld, "story.motionHandheld"),
];

const STAGES: [&str; 5] = [
    "story.stageScript",
    "story.stageCast",
    "story.stageBoard",
    "story.stageGenerate",
    "story.stageReview",
];

struct Controller {
    /// The project folder the story belongs to; empty when none is open.
    project: String,
    story: Option<Story>,
    queue: Option<Queue>,
    scene: usize,
    shot: Option<usize>,
    /// What to tell the person, such as a story file that could not be read.
    notice: String,
}

thread_local! {
    static CONTROLLER: RefCell<Option<Controller>> = const { RefCell::new(None) };
}

fn with(body: impl FnOnce(&mut Controller, &Shell, &App)) {
    Shell::with(|shell, app| {
        CONTROLLER.with(|slot| {
            if let Some(controller) = slot.borrow_mut().as_mut() {
                body(controller, shell, &app);
            }
        });
    });
}

/// "林夏：哪来的?" as a line: who, and what is said. A line with no name
/// before a colon is the narrator's.
fn parse_line(text: &str) -> Option<Line> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // Only the first colon can mark a name.
    if let Some((at, colon)) = text.char_indices().find(|(_, c)| *c == '：' || *c == ':') {
        let who = text[..at].trim();
        if !who.is_empty() && who.chars().count() <= 12 {
            return Some(Line {
                who: who.to_owned(),
                text: text[at + colon.len_utf8()..].trim().to_owned(),
            });
        }
    }
    Some(Line { who: String::new(), text: text.to_owned() })
}

fn show_line(line: &Option<Line>) -> String {
    match line {
        None => String::new(),
        Some(line) if line.who.is_empty() => line.text.clone(),
        Some(line) => format!("{}：{}", line.who, line.text),
    }
}

fn clock(seconds: f32) -> String {
    let whole = seconds.round() as u32;
    if whole >= 60 { format!("{}:{:02}", whole / 60, whole % 60) } else { format!("{whole}s") }
}

impl Controller {
    fn folder(&self) -> PathBuf {
        PathBuf::from(&self.project)
    }

    /// Brings the story of the open project up, starting its queue.
    fn enter(&mut self, shell: &Shell, app: &App) {
        let (path, name) = {
            let studio = shell.studio.borrow();
            match studio.session.as_ref() {
                Some(session) => (session.path().to_owned(), studio.project_name.clone()),
                None => (String::new(), String::new()),
            }
        };
        if path != self.project || (self.story.is_none() && self.notice.is_empty()) {
            self.project = path;
            self.story = None;
            self.queue = None;
            self.notice.clear();
            self.scene = 0;
            self.shot = None;
            if !self.project.is_empty() {
                match store::load(&self.folder()) {
                    Ok(Some(story)) => self.story = Some(story),
                    Ok(None) => {
                        let title = if name.is_empty() { t("story.untitled") } else { name };
                        self.story = Some(Story::new(&title));
                    }
                    // Not replaced by an empty story, which the next edit
                    // would write over the one that could not be read.
                    Err(error) => self.notice = error,
                }
                if self.story.is_some() {
                    self.queue = Some(Queue::start(
                        Arc::new(Rehearsal { delay: Duration::from_millis(1600) }),
                        self.folder(),
                        2,
                    ));
                }
            }
        }
        self.publish(app);
    }

    fn save(&self) {
        if let Some(story) = &self.story
            && let Err(error) = store::save(&self.folder(), story)
        {
            log::warn!("story: could not be saved: {error}");
        }
    }

    fn shot_ids(&self) -> Vec<String> {
        self.story
            .as_ref()
            .and_then(|story| story.scenes.get(self.scene))
            .map(|scene| scene.shots.iter().map(|shot| shot.id.clone()).collect())
            .unwrap_or_default()
    }

    fn current_id(&self) -> Option<String> {
        self.shot_ids().get(self.shot?).cloned()
    }

    fn card(&self, scene_number: usize, index: usize, shot: &concat_drama::Shot) -> StoryShot {
        let (phase, progress) = match &shot.frame.phase {
            Phase::Idle => (0, 0.0),
            Phase::Queued => (1, 0.0),
            Phase::Running { progress } => (2, *progress),
            Phase::Done => (3, 1.0),
            Phase::Failed { .. } => (4, 0.0),
        };
        let note = match &shot.frame.phase {
            Phase::Failed { reason } => reason.clone(),
            Phase::Idle if shot.action.trim().is_empty() => t("story.needsAction"),
            _ => String::new(),
        };
        let frame = shot
            .frame
            .current()
            .and_then(|take| slint::Image::load_from_path(&self.folder().join(&take.path)).ok());
        StoryShot {
            id: shot.id.as_str().into(),
            number: format!("{}-{}", scene_number + 1, index + 1).into(),
            action: shot.action.as_str().into(),
            line: show_line(&shot.line).into(),
            who: shot.line.as_ref().map(|l| l.who.as_str()).unwrap_or("").into(),
            size: SIZES.iter().position(|(s, _)| *s == shot.size).unwrap_or(2) as i32,
            motion: MOTIONS.iter().position(|(m, _)| *m == shot.motion).unwrap_or(0) as i32,
            seconds: shot.seconds,
            phase,
            progress,
            has_frame: frame.is_some(),
            frame: frame.unwrap_or_default(),
            note: note.into(),
        }
    }

    fn publish(&self, app: &App) {
        let state = app.global::<StoryState>();
        let strings = |keys: &[&str]| {
            ModelRc::new(VecModel::from(
                keys.iter().map(|key| SharedString::from(t(key))).collect::<Vec<_>>(),
            ))
        };
        state.set_stages(strings(&STAGES));
        state.set_sizes(strings(&SIZES.map(|(_, key)| key)));
        state.set_motions(strings(&MOTIONS.map(|(_, key)| key)));
        state.set_notice(self.notice.as_str().into());

        let Some(story) = &self.story else {
            state.set_title("".into());
            state.set_summary(self.notice.as_str().into());
            state.set_scenes(ModelRc::new(VecModel::from(Vec::<StoryScene>::new())));
            state.set_shots(ModelRc::new(VecModel::from(Vec::<StoryShot>::new())));
            state.set_has_current(false);
            return;
        };
        let scene = self.scene.min(story.scenes.len().saturating_sub(1));
        state.set_title(story.title.as_str().into());
        state.set_summary(
            tf(
                "story.summary",
                &[&story.shots().count(), &clock(story.seconds())],
            )
            .into(),
        );
        state.set_scenes(ModelRc::new(VecModel::from(
            story
                .scenes
                .iter()
                .map(|s| StoryScene {
                    id: s.id.as_str().into(),
                    title: s.title.as_str().into(),
                    shots: s.shots.len() as i32,
                    seconds: clock(s.shots.iter().map(|shot| shot.seconds).sum()).into(),
                })
                .collect::<Vec<_>>(),
        )));
        state.set_scene(if story.scenes.is_empty() { -1 } else { scene as i32 });
        let cards: Vec<StoryShot> = story
            .scenes
            .get(scene)
            .map(|s| {
                s.shots
                    .iter()
                    .enumerate()
                    .map(|(i, shot)| self.card(scene, i, shot))
                    .collect()
            })
            .unwrap_or_default();
        let shot = self.shot.filter(|i| *i < cards.len());
        match shot.and_then(|i| cards.get(i)) {
            Some(card) => {
                state.set_current(card.clone());
                state.set_has_current(true);
            }
            None => state.set_has_current(false),
        }
        state.set_shot(shot.map(|i| i as i32).unwrap_or(-1));
        state.set_shots(ModelRc::new(VecModel::from(cards)));
    }

    fn edit(&mut self, app: &App, change: impl FnOnce(&mut concat_drama::Shot)) {
        let Some(id) = self.current_id() else { return };
        if let Some(shot) = self.story.as_mut().and_then(|story| story.shot_mut(&id)) {
            change(shot);
            self.save();
            self.publish(app);
        }
    }

    /// Asks for a shot's first frame; what is missing is shown on the shot.
    fn make_frame(&mut self, id: &str) {
        let (Some(story), Some(queue)) = (self.story.as_mut(), self.queue.as_ref()) else { return };
        let target = Target { id: id.to_owned(), kind: Kind::Frame };
        match prompt::shot_request(story, id, Kind::Frame, &PathBuf::from(&self.project)) {
            Ok(request) => {
                jobs::mark_queued(story, &target);
                queue.submit(target, request);
            }
            Err(missing) => {
                if let Some(shot) = story.shot_mut(id) {
                    shot.frame.phase = Phase::Failed { reason: missing.0 };
                }
            }
        }
    }

    /// Applies what the queue reported; true if anything changed.
    fn drain(&mut self) -> bool {
        let Some(queue) = &self.queue else { return false };
        let events: Vec<Event> = queue.events();
        if events.is_empty() {
            return false;
        }
        let mut finished = false;
        if let Some(story) = self.story.as_mut() {
            for event in &events {
                jobs::apply(story, event);
                finished |= matches!(event, Event::Done(..) | Event::Failed(..));
            }
        }
        if finished {
            self.save();
        }
        true
    }
}

/// Sets the screen up: its callbacks, and the timer that applies what the
/// queue reports.
pub fn install(app: &App) {
    CONTROLLER.with(|slot| {
        *slot.borrow_mut() = Some(Controller {
            project: String::new(),
            story: None,
            queue: None,
            scene: 0,
            shot: None,
            notice: String::new(),
        });
    });
    let state = app.global::<StoryState>();
    state.on_entered(|| with(|c, shell, app| c.enter(shell, app)));
    state.on_stage_selected(|index| {
        with(|_, _, app| app.global::<StoryState>().set_stage(index));
    });
    state.on_scene_selected(|index| {
        with(|c, _, app| {
            c.scene = index.max(0) as usize;
            c.shot = None;
            c.publish(app);
        });
    });
    state.on_shot_selected(|index| {
        with(|c, _, app| {
            c.shot = Some(index.max(0) as usize);
            c.publish(app);
        });
    });
    state.on_add_scene(|| {
        with(|c, _, app| {
            if let Some(story) = c.story.as_mut() {
                let number = story.scenes.len() + 1;
                story.add_scene(&tf("story.newScene", &[&number]));
                c.scene = story.scenes.len() - 1;
                c.shot = None;
                c.save();
                c.publish(app);
            }
        });
    });
    state.on_add_shot(|| {
        with(|c, _, app| {
            let scene = c.scene;
            if let Some(story) = c.story.as_mut()
                && let Some(id) = story.scenes.get(scene).map(|s| s.id.clone())
                && story.add_shot(&id, "", 5.0).is_some()
            {
                c.shot = Some(story.scenes[scene].shots.len() - 1);
                c.save();
                c.publish(app);
            }
        });
    });
    state.on_delete_shot(|| {
        with(|c, _, app| {
            let (Some(id), scene) = (c.current_id(), c.scene) else { return };
            if let Some(story) = c.story.as_mut()
                && let Some(s) = story.scenes.get_mut(scene)
            {
                s.shots.retain(|shot| shot.id != id);
                c.shot = None;
                c.save();
                c.publish(app);
            }
        });
    });
    state.on_action_edited(|text| {
        with(|c, _, app| c.edit(app, |shot| shot.action = text.to_string()));
    });
    state.on_line_edited(|text| {
        with(|c, _, app| c.edit(app, |shot| shot.line = parse_line(&text)));
    });
    state.on_size_changed(|index| {
        with(|c, _, app| {
            if let Some((size, _)) = SIZES.get(index.max(0) as usize) {
                let size = *size;
                c.edit(app, |shot| shot.size = size);
            }
        });
    });
    state.on_motion_changed(|index| {
        with(|c, _, app| {
            if let Some((motion, _)) = MOTIONS.get(index.max(0) as usize) {
                let motion = *motion;
                c.edit(app, |shot| shot.motion = motion);
            }
        });
    });
    state.on_seconds_changed(|seconds| {
        with(|c, _, app| c.edit(app, |shot| shot.seconds = seconds));
    });
    state.on_generate_frame(|| {
        with(|c, _, app| {
            if let Some(id) = c.current_id() {
                c.make_frame(&id);
                c.publish(app);
            }
        });
    });
    state.on_generate_all(|| {
        with(|c, _, app| {
            let ids: Vec<String> = c
                .story
                .as_ref()
                .map(|story| {
                    story
                        .shots()
                        .filter(|shot| {
                            !matches!(shot.frame.phase, Phase::Queued | Phase::Running { .. })
                                && shot.frame.current().is_none()
                                && !shot.action.trim().is_empty()
                        })
                        .map(|shot| shot.id.clone())
                        .collect()
                })
                .unwrap_or_default();
            for id in ids {
                c.make_frame(&id);
            }
            c.publish(app);
        });
    });

    // What the queue reports is applied here, a few times a second while
    // anything is being made.
    let tick = slint::Timer::default();
    tick.start(slint::TimerMode::Repeated, Duration::from_millis(250), || {
        with(|c, _, app| {
            if c.drain() {
                c.publish(app);
            }
        });
    });
    std::mem::forget(tick);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_before_a_colon_is_who_speaks() {
        let line = parse_line("林夏：这块表,哪来的?").unwrap();
        assert_eq!((line.who.as_str(), line.text.as_str()), ("林夏", "这块表,哪来的?"));
        let line = parse_line("Xia: where from?").unwrap();
        assert_eq!((line.who.as_str(), line.text.as_str()), ("Xia", "where from?"));
    }

    #[test]
    fn a_line_without_a_name_is_the_narrators() {
        let line = parse_line("雨下了一整夜").unwrap();
        assert!(line.who.is_empty());
        assert_eq!(line.text, "雨下了一整夜");
        // A colon far into a sentence is not a name.
        let line = parse_line("It was the night before the storm: nobody slept").unwrap();
        assert!(line.who.is_empty());
        assert_eq!(parse_line("   "), None);
    }

    #[test]
    fn a_line_is_shown_the_way_it_is_typed() {
        for text in ["林夏：哪来的?", "雨下了一整夜"] {
            assert_eq!(show_line(&parse_line(text)), text);
        }
        assert_eq!(show_line(&None), "");
    }

    #[test]
    fn a_running_time_reads_in_seconds_and_then_in_minutes() {
        assert_eq!(clock(5.0), "5s");
        assert_eq!(clock(58.4), "58s");
        assert_eq!(clock(61.0), "1:01");
    }
}
