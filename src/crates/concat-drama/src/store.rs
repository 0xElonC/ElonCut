// SPDX-License-Identifier: AGPL-3.0-or-later

//! Keeping the story in the project folder, beside the project itself.

use std::path::{Path, PathBuf};

use crate::model::{Phase, Story};

/// The story's file in a project folder.
pub fn file(project: &Path) -> PathBuf {
    project.join("story.json")
}

/// The folder generated files go in.
pub fn assets(project: &Path) -> PathBuf {
    project.join("story")
}

/// The story of a project: `None` when the project has none, an error when
/// the file is there and cannot be read, so it is never overwritten by an
/// empty one.
pub fn load(project: &Path) -> Result<Option<Story>, String> {
    let path = file(project);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut story: Story =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    heal(&mut story);
    Ok(Some(story))
}

/// A story from a run that ended in the middle of a job has pieces that say
/// they are working and are not: they go back to waiting for the person.
fn heal(story: &mut Story) {
    let stale = |phase: &mut Phase| {
        if matches!(phase, Phase::Queued | Phase::Running { .. }) {
            *phase = Phase::Idle;
        }
    };
    for character in &mut story.cast {
        stale(&mut character.portrait.phase);
        stale(&mut character.turnaround.phase);
    }
    for scene in &mut story.scenes {
        for shot in &mut scene.shots {
            stale(&mut shot.frame.phase);
            stale(&mut shot.video.phase);
            stale(&mut shot.voice.phase);
        }
    }
}

/// Writes the story: to a side file first and then over the real one, so a
/// crash in the middle leaves the old story and not half of the new.
pub fn save(project: &Path, story: &Story) -> Result<(), String> {
    let path = file(project);
    let side = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(story).map_err(|e| e.to_string())?;
    std::fs::write(&side, text).map_err(|e| format!("{}: {e}", side.display()))?;
    std::fs::rename(&side, &path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Take;

    #[test]
    fn a_story_comes_back_as_it_was_saved() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Ok(None));

        let mut story = Story::new("雨夜追凶");
        let c = story.add_character("林夏", "detective", "short hair");
        let s = story.add_scene("shop");
        let h = story.add_shot(&s, "stares", 5.0).unwrap();
        story.shot_mut(&h).unwrap().cast = vec![c];
        story.shot_mut(&h).unwrap().frame.add(Take {
            path: "story/h3.png".into(),
            service: "x".into(),
            made: 7,
            seconds: None,
        });
        save(dir.path(), &story).unwrap();
        assert_eq!(load(dir.path()).unwrap(), Some(story));
    }

    #[test]
    fn work_in_flight_when_the_app_closed_is_not_shown_as_working() {
        let dir = tempfile::tempdir().unwrap();
        let mut story = Story::new("t");
        let s = story.add_scene("a");
        let h = story.add_shot(&s, "x", 1.0).unwrap();
        story.shot_mut(&h).unwrap().video.phase = Phase::Running { progress: 0.4 };
        story.shot_mut(&h).unwrap().frame.phase = Phase::Queued;
        save(dir.path(), &story).unwrap();
        let back = load(dir.path()).unwrap().unwrap();
        let shot = back.shot(&h).unwrap().1;
        assert_eq!(shot.video.phase, Phase::Idle);
        assert_eq!(shot.frame.phase, Phase::Idle);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_and_not_an_empty_story() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(file(dir.path()), "{ not json").unwrap();
        assert!(load(dir.path()).is_err());
    }

    #[test]
    fn ids_are_not_handed_out_twice_after_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut story = Story::new("t");
        let a = story.add_scene("a");
        save(dir.path(), &story).unwrap();
        let mut back = load(dir.path()).unwrap().unwrap();
        assert_ne!(back.add_scene("b"), a);
    }
}
