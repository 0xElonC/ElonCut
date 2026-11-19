// SPDX-License-Identifier: AGPL-3.0-or-later

//! The story: scenes made of shots, the cast that appears in them, and what
//! has been generated for each piece.

use serde::{Deserialize, Serialize};

/// How much of the subject a shot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotSize {
    /// The whole place, the people small in it.
    Wide,
    /// People head to toe.
    Full,
    /// People from the waist up.
    #[default]
    Medium,
    /// Head and shoulders.
    Close,
    /// One detail: a hand, an object, an eye.
    Detail,
}

impl ShotSize {
    /// The words a generator is told.
    pub fn phrase(self) -> &'static str {
        match self {
            ShotSize::Wide => "wide establishing shot",
            ShotSize::Full => "full shot",
            ShotSize::Medium => "medium shot",
            ShotSize::Close => "close-up",
            ShotSize::Detail => "extreme close-up on a detail",
        }
    }
}

/// How the camera moves during a shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    /// Locked off.
    #[default]
    Still,
    /// Towards the subject.
    PushIn,
    /// Away from it.
    PullOut,
    /// Turning on the spot.
    Pan,
    /// Sliding sideways.
    Track,
    /// Following a person.
    Follow,
    /// Circling the subject.
    Orbit,
    /// Held by hand, a little unsteady.
    Handheld,
}

impl Motion {
    /// The words a generator is told; empty for a locked-off camera.
    pub fn phrase(self) -> &'static str {
        match self {
            Motion::Still => "static camera",
            Motion::PushIn => "slow push in",
            Motion::PullOut => "slow pull out",
            Motion::Pan => "slow pan",
            Motion::Track => "lateral tracking shot",
            Motion::Follow => "camera follows the subject",
            Motion::Orbit => "camera orbits the subject",
            Motion::Handheld => "handheld camera",
        }
    }
}

/// What a generated piece is doing.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Phase {
    /// Nothing asked for yet.
    #[default]
    Idle,
    /// Waiting for a turn in the queue.
    Queued,
    /// A service is working on it; `progress` is 0 to 1 where it says.
    Running {
        /// How far along, when the service reports it.
        progress: f32,
    },
    /// Made.
    Done,
    /// The service said no, or could not be reached.
    Failed {
        /// Why, in the service's words or ours.
        reason: String,
    },
}

/// One generated file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Take {
    /// Where it is, relative to the project folder.
    pub path: String,
    /// What made it.
    pub service: String,
    /// When, in seconds since the epoch.
    pub made: u64,
    /// For audio and video, how long it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
}

/// A piece that is asked for, retried and chosen between: a shot's first
/// frame, its video, or its voice.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Slot {
    /// What it is doing now.
    pub phase: Phase,
    /// Everything made so far, oldest first.
    pub takes: Vec<Take>,
    /// Which take is used.
    pub chosen: Option<usize>,
}

impl Slot {
    /// The take in use.
    pub fn current(&self) -> Option<&Take> {
        self.takes.get(self.chosen?)
    }

    /// Adds a take and chooses it.
    pub fn add(&mut self, take: Take) {
        self.takes.push(take);
        self.chosen = Some(self.takes.len() - 1);
        self.phase = Phase::Done;
    }
}

/// What a character is called in a line, and which voice says it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Voice {
    /// The service's id for the voice, empty until one is chosen.
    pub id: String,
    /// A name for the person to read.
    pub name: String,
    /// Speed, 1.0 being the voice's own.
    #[serde(default = "one")]
    pub speed: f32,
}

fn one() -> f32 {
    1.0
}

/// A person in the story.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Character {
    /// Stable id, `c1`, `c2`...
    pub id: String,
    /// The name.
    pub name: String,
    /// Who they are in the story.
    pub role: String,
    /// What they look like, sent with every shot they are in.
    pub look: String,
    /// Pictures the person gave, relative to the project folder.
    #[serde(default)]
    pub references: Vec<String>,
    /// The costume picture.
    #[serde(default)]
    pub portrait: Slot,
    /// Front, side and back, made from the portrait.
    #[serde(default)]
    pub turnaround: Slot,
    /// Their voice.
    #[serde(default)]
    pub voice: Voice,
    /// A locked character's portrait goes with every shot they are in.
    #[serde(default)]
    pub locked: bool,
}

/// What is said in a shot.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Line {
    /// Who says it; empty for a narrator.
    pub who: String,
    /// What is said.
    pub text: String,
}

/// One shot: the unit that is generated and then cut.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Shot {
    /// Stable id, `h1`, `h2`...
    pub id: String,
    /// What happens.
    pub action: String,
    /// What is said, if anything.
    #[serde(default)]
    pub line: Option<Line>,
    /// How near.
    #[serde(default)]
    pub size: ShotSize,
    /// How the camera moves.
    #[serde(default)]
    pub motion: Motion,
    /// Length in seconds the picture is asked for.
    pub seconds: f32,
    /// Who is in it, by character id.
    #[serde(default)]
    pub cast: Vec<String>,
    /// The first frame.
    #[serde(default)]
    pub frame: Slot,
    /// The moving picture.
    #[serde(default)]
    pub video: Slot,
    /// The spoken line.
    #[serde(default)]
    pub voice: Slot,
    /// The person's own words for the generator, used instead of the made ones.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prompt: String,
}

/// A place and a stretch of time in the story.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Scene {
    /// Stable id, `s1`, `s2`...
    pub id: String,
    /// "老周的店".
    pub title: String,
    /// What happens, in a sentence.
    #[serde(default)]
    pub summary: String,
    /// Where and when, for the generator: "interior, an old shop, night, rain".
    #[serde(default)]
    pub setting: String,
    /// The script text of the scene.
    #[serde(default)]
    pub script: String,
    /// The shots, in order.
    #[serde(default)]
    pub shots: Vec<Shot>,
}

/// The whole story of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Story {
    /// The file format's version.
    pub version: u32,
    /// The title.
    pub title: String,
    /// One sentence.
    #[serde(default)]
    pub logline: String,
    /// The script as it was written, which the scenes are split from.
    #[serde(default)]
    pub script: String,
    /// The look of every picture: "cinematic, rainy night, film grain".
    #[serde(default)]
    pub style: String,
    /// Width and height of the frame; 9:16 for a vertical drama.
    pub frame: (u32, u32),
    /// The people.
    #[serde(default)]
    pub cast: Vec<Character>,
    /// The scenes, in order.
    #[serde(default)]
    pub scenes: Vec<Scene>,
    /// Counter behind the ids handed out.
    #[serde(default)]
    pub next_id: u32,
}

impl Default for Story {
    fn default() -> Self {
        Story {
            version: 1,
            title: String::new(),
            logline: String::new(),
            script: String::new(),
            style: String::new(),
            frame: (1080, 1920),
            cast: Vec::new(),
            scenes: Vec::new(),
            next_id: 1,
        }
    }
}

/// Which slot of a shot, or of a character, a job fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A character's costume picture.
    Portrait,
    /// A shot's first frame.
    Frame,
    /// A shot's moving picture.
    Video,
    /// A shot's spoken line.
    Voice,
}

impl Story {
    /// A new story named `title`.
    pub fn new(title: &str) -> Story {
        Story {
            title: title.to_owned(),
            ..Story::default()
        }
    }

    pub(crate) fn fresh(&mut self, prefix: char) -> String {
        let id = format!("{prefix}{}", self.next_id);
        self.next_id += 1;
        id
    }

    /// Adds a character and returns its id.
    pub fn add_character(&mut self, name: &str, role: &str, look: &str) -> String {
        let id = self.fresh('c');
        self.cast.push(Character {
            id: id.clone(),
            name: name.to_owned(),
            role: role.to_owned(),
            look: look.to_owned(),
            voice: Voice { speed: 1.0, ..Voice::default() },
            ..Character::default()
        });
        id
    }

    /// Adds a scene and returns its id.
    pub fn add_scene(&mut self, title: &str) -> String {
        let id = self.fresh('s');
        self.scenes.push(Scene {
            id: id.clone(),
            title: title.to_owned(),
            ..Scene::default()
        });
        id
    }

    /// Adds a shot at the end of a scene and returns its id; `None` when
    /// there is no such scene.
    pub fn add_shot(&mut self, scene: &str, action: &str, seconds: f32) -> Option<String> {
        let id = self.fresh('h');
        let scene = self.scenes.iter_mut().find(|s| s.id == scene)?;
        scene.shots.push(Shot {
            id: id.clone(),
            action: action.to_owned(),
            seconds,
            ..Shot::default()
        });
        Some(id)
    }

    /// A character by id.
    pub fn character(&self, id: &str) -> Option<&Character> {
        self.cast.iter().find(|c| c.id == id)
    }

    /// A character by name, for a line that says who speaks.
    pub fn character_named(&self, name: &str) -> Option<&Character> {
        self.cast.iter().find(|c| c.name == name)
    }

    /// A shot and the scene it is in.
    pub fn shot(&self, id: &str) -> Option<(&Scene, &Shot)> {
        self.scenes
            .iter()
            .find_map(|scene| scene.shots.iter().find(|s| s.id == id).map(|s| (scene, s)))
    }

    /// A shot, to change.
    pub fn shot_mut(&mut self, id: &str) -> Option<&mut Shot> {
        self.scenes
            .iter_mut()
            .flat_map(|scene| scene.shots.iter_mut())
            .find(|s| s.id == id)
    }

    /// Every shot in story order.
    pub fn shots(&self) -> impl Iterator<Item = &Shot> {
        self.scenes.iter().flat_map(|scene| scene.shots.iter())
    }

    /// The running time if every shot plays for its length.
    pub fn seconds(&self) -> f32 {
        self.shots().map(|shot| shot.seconds).sum()
    }
}
