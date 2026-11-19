// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a service is asked for each piece: the words and the pictures that
//! go with them, made from the story so that the person writes a shot once.

use std::path::{Path, PathBuf};

use crate::model::{Kind, Shot, Story, Voice};

/// What is asked of a service for one piece.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// What is made.
    pub kind: Kind,
    /// The words for a picture or a video; empty for a voice.
    pub prompt: String,
    /// What is spoken, for a voice.
    pub speech: String,
    /// The voice to speak it in.
    pub voice: Voice,
    /// Pictures the result must look like: locked characters' portraits and
    /// the people's own pictures. Absolute paths.
    pub references: Vec<PathBuf>,
    /// For a video, the first frame it starts from.
    pub first_frame: Option<PathBuf>,
    /// The frame's width and height.
    pub frame: (u32, u32),
    /// For a video, how long it is asked to run.
    pub seconds: f32,
}

/// Why a request cannot be made yet, in words for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing(pub String);

fn join(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The words for a shot's first frame.
pub fn frame_prompt(story: &Story, shot: &Shot) -> String {
    if !shot.prompt.trim().is_empty() {
        return shot.prompt.trim().to_owned();
    }
    let setting = story
        .scenes
        .iter()
        .find(|scene| scene.shots.iter().any(|s| s.id == shot.id))
        .map(|scene| scene.setting.as_str())
        .unwrap_or("");
    let people: Vec<String> = shot
        .cast
        .iter()
        .filter_map(|id| story.character(id))
        .map(|c| format!("{} ({})", c.name, c.look.trim()))
        .collect();
    let people = people.join("; ");
    join(&[&shot.action, &people, setting, shot.size.phrase(), &story.style])
}

/// The words for a shot's video: what moves, and how the camera does.
pub fn video_prompt(story: &Story, shot: &Shot) -> String {
    let mut words = join(&[&shot.action, shot.motion.phrase(), &story.style]);
    if let Some(line) = shot.line.as_ref().filter(|line| !line.who.is_empty()) {
        words.push_str(&format!(", {} is speaking", line.who));
    }
    words
}

fn picture_of(project: &Path, path: &str) -> PathBuf {
    project.join(path)
}

/// The pictures a shot's frame must look like.
pub fn references(story: &Story, shot: &Shot, project: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for id in &shot.cast {
        let Some(character) = story.character(id) else { continue };
        if let Some(take) = character.portrait.current().filter(|_| character.locked) {
            found.push(picture_of(project, &take.path));
            continue;
        }
        found.extend(character.references.iter().map(|p| picture_of(project, p)));
    }
    found
}

/// The request for a shot's piece, or what is missing for it.
pub fn shot_request(
    story: &Story,
    shot_id: &str,
    kind: Kind,
    project: &Path,
) -> Result<Request, Missing> {
    let (_, shot) = story
        .shot(shot_id)
        .ok_or_else(|| Missing(format!("no shot {shot_id}")))?;
    let mut request = Request {
        kind,
        prompt: String::new(),
        speech: String::new(),
        voice: Voice::default(),
        references: Vec::new(),
        first_frame: None,
        frame: story.frame,
        seconds: shot.seconds,
    };
    match kind {
        Kind::Frame => {
            if shot.action.trim().is_empty() && shot.prompt.trim().is_empty() {
                return Err(Missing("the shot has no action yet".into()));
            }
            request.prompt = frame_prompt(story, shot);
            request.references = references(story, shot, project);
        }
        Kind::Video => {
            let frame = shot
                .frame
                .current()
                .ok_or_else(|| Missing("the first frame is not made yet".into()))?;
            request.first_frame = Some(picture_of(project, &frame.path));
            request.prompt = video_prompt(story, shot);
        }
        Kind::Voice => {
            let line = shot
                .line
                .as_ref()
                .filter(|line| !line.text.trim().is_empty())
                .ok_or_else(|| Missing("the shot has no line to speak".into()))?;
            let speaker = story.character_named(&line.who);
            let voice = speaker.map(|c| c.voice.clone()).unwrap_or_default();
            if voice.id.is_empty() {
                let who = if line.who.is_empty() { "the narrator" } else { &line.who };
                return Err(Missing(format!("{who} has no voice chosen")));
            }
            request.speech = line.text.trim().to_owned();
            request.voice = voice;
        }
        Kind::Portrait => return Err(Missing("a portrait belongs to a character".into())),
    }
    Ok(request)
}

/// The request for a character's costume picture.
pub fn portrait_request(story: &Story, character_id: &str, project: &Path) -> Result<Request, Missing> {
    let character = story
        .character(character_id)
        .ok_or_else(|| Missing(format!("no character {character_id}")))?;
    if character.look.trim().is_empty() {
        return Err(Missing(format!("{} has no description yet", character.name)));
    }
    Ok(Request {
        kind: Kind::Portrait,
        prompt: join(&[
            "character design, full body, neutral pose, plain background",
            &character.look,
            &story.style,
        ]),
        speech: String::new(),
        voice: Voice::default(),
        references: character.references.iter().map(|p| picture_of(project, p)).collect(),
        first_frame: None,
        frame: (1024, 1536),
        seconds: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Line, Take};

    fn story() -> (Story, String, String) {
        let mut story = Story::new("t");
        story.style = "cinematic, rain".into();
        let c = story.add_character("林夏", "detective", "short hair, black coat");
        story.add_character("老周", "shopkeeper", "old man");
        let s = story.add_scene("shop");
        story.scenes[0].setting = "interior, old shop, night".into();
        let h = story.add_shot(&s, "Xia stares at the watch", 5.0).unwrap();
        story.shot_mut(&h).unwrap().cast = vec![c.clone()];
        (story, c, h)
    }

    #[test]
    fn the_frame_prompt_carries_action_look_setting_size_and_style() {
        let (story, _, h) = story();
        let words = frame_prompt(&story, story.shot(&h).unwrap().1);
        assert!(words.starts_with("Xia stares at the watch"));
        assert!(words.contains("林夏 (short hair, black coat)"));
        assert!(words.contains("interior, old shop, night"));
        assert!(words.contains("medium shot"));
        assert!(words.ends_with("cinematic, rain"));
    }

    #[test]
    fn a_prompt_the_person_wrote_is_used_as_it_is() {
        let (mut story, _, h) = story();
        story.shot_mut(&h).unwrap().prompt = "  my words ".into();
        assert_eq!(frame_prompt(&story, story.shot(&h).unwrap().1), "my words");
    }

    #[test]
    fn a_locked_character_goes_as_their_portrait_and_an_unlocked_one_as_their_pictures() {
        let (mut story, c, h) = story();
        story.cast[0].references = vec!["refs/a.png".into()];
        let project = Path::new("/p");
        let shot = story.shot(&h).unwrap().1.clone();
        assert_eq!(references(&story, &shot, project), vec![PathBuf::from("/p/refs/a.png")]);

        story.cast[0].portrait.add(Take {
            path: "story/c1.png".into(),
            service: "x".into(),
            made: 1,
            seconds: None,
        });
        story.cast[0].locked = true;
        assert_eq!(references(&story, &shot, project), vec![PathBuf::from("/p/story/c1.png")]);
        assert_eq!(story.cast[0].id, c);
    }

    #[test]
    fn a_video_needs_its_first_frame_and_a_voice_needs_a_voice() {
        let (mut story, _, h) = story();
        let project = Path::new("/p");
        let wanted = shot_request(&story, &h, Kind::Video, project).unwrap_err();
        assert!(wanted.0.contains("first frame"));

        story.shot_mut(&h).unwrap().line = Some(Line { who: "林夏".into(), text: "哪来的?".into() });
        let wanted = shot_request(&story, &h, Kind::Voice, project).unwrap_err();
        assert!(wanted.0.contains("林夏 has no voice"));

        story.cast[0].voice.id = "v1".into();
        let request = shot_request(&story, &h, Kind::Voice, project).unwrap();
        assert_eq!(request.speech, "哪来的?");
        assert_eq!(request.voice.id, "v1");
    }

    #[test]
    fn a_video_starts_from_the_chosen_frame() {
        let (mut story, _, h) = story();
        story.shot_mut(&h).unwrap().motion = crate::model::Motion::PushIn;
        story.shot_mut(&h).unwrap().frame.add(Take {
            path: "story/h3.png".into(),
            service: "x".into(),
            made: 1,
            seconds: None,
        });
        let request = shot_request(&story, &h, Kind::Video, Path::new("/p")).unwrap();
        assert_eq!(request.first_frame, Some(PathBuf::from("/p/story/h3.png")));
        assert!(request.prompt.contains("slow push in"));
    }

    #[test]
    fn a_portrait_wants_a_description() {
        let (mut story, c, _) = story();
        assert!(portrait_request(&story, &c, Path::new("/p")).is_ok());
        story.cast[0].look.clear();
        assert!(portrait_request(&story, &c, Path::new("/p")).is_err());
    }
}
