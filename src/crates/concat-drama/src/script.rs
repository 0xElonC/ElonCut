// SPDX-License-Identifier: AGPL-3.0-or-later

//! From the script's words to scenes of shots.
//!
//! The script is plain text. A line that opens with "场景" or "Scene" (or
//! `#`) starts a scene; "Name: words" is somebody speaking; any other line
//! is something that happens. Each happening is a shot, and a spoken line
//! right after one belongs to it. Splitting again after an edit keeps every
//! shot whose words did not change, so what was already made for it stays.

use crate::model::{Line, Scene, Shot, Story};

/// What splitting a script does, or would do, to the storyboard.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Plan {
    /// Scenes the script has.
    pub scenes: usize,
    /// Shots the script has.
    pub shots: usize,
    /// Their running time.
    pub seconds: f32,
    /// Shots that stay as they are, with what was made for them.
    pub kept: usize,
    /// Shots in the storyboard that the script no longer has.
    pub removed: usize,
}

struct Draft {
    title: String,
    shots: Vec<(String, Option<Line>)>,
}

/// "林夏：哪来的?" as a line: who, and what is said. A line with no name
/// before a colon is the narrator's.
pub fn parse_line(text: &str) -> Option<Line> {
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

/// The scene a heading line names, if it is one; the name may be empty.
fn heading(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let rest = if let Some(rest) = line.strip_prefix('#') {
        rest.trim_start_matches('#')
    } else {
        let prefix = ["场景", "場景", "scene", "int.", "ext."]
            .into_iter()
            .find(|p| lower.starts_with(p))?;
        // Taken from `line`, not `lower`: lowering can change byte lengths.
        let skip: usize = line.chars().take(prefix.chars().count()).map(char::len_utf8).sum();
        &line[skip..]
    };
    let rest = rest.trim_start_matches(|c: char| {
        c.is_ascii_digit() || "一二三四五六七八九十 \t".contains(c)
    });
    Some(
        rest.trim_matches(|c: char| c.is_whitespace() || ":：、.-—".contains(c))
            .to_owned(),
    )
}

fn drafts(text: &str) -> Vec<Draft> {
    let mut out: Vec<Draft> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(title) = heading(line) {
            out.push(Draft { title, shots: Vec::new() });
            continue;
        }
        if out.is_empty() {
            out.push(Draft { title: String::new(), shots: Vec::new() });
        }
        let draft = out.last_mut().expect("a scene was just pushed");
        match parse_line(line) {
            Some(spoken) if !spoken.who.is_empty() => {
                // Said right after something happens: the same shot.
                match draft.shots.last_mut() {
                    Some((action, said)) if said.is_none() && !action.is_empty() => *said = Some(spoken),
                    _ => draft.shots.push((String::new(), Some(spoken))),
                }
            }
            _ => draft.shots.push((line.to_owned(), None)),
        }
    }
    out
}

/// A length for a new shot: spoken words at about four a second, and never
/// less than the picture needs to read.
fn seconds_for(action: &str, line: &Option<Line>) -> f32 {
    let spoken = line
        .as_ref()
        .map(|l| (l.text.chars().count() as f32 / 4.0 + 1.0).ceil())
        .unwrap_or(0.0);
    let floor = if action.is_empty() { 3.0 } else { 5.0 };
    spoken.max(floor).min(15.0)
}

/// Makes the story's scenes from `text`, which is kept as its script.
/// People who speak and are not in the cast yet are added to it.
/// `untitled(n)` names the n-th scene when the script does not.
pub fn apply(story: &mut Story, text: &str, untitled: &dyn Fn(usize) -> String) -> Plan {
    story.script = text.to_owned();
    let mut old = std::mem::take(&mut story.scenes);
    let before: usize = old.iter().map(|s| s.shots.len()).sum();
    let mut pool: Vec<Shot> = old.iter_mut().flat_map(|s| std::mem::take(&mut s.shots)).collect();
    let mut kept = 0;

    for (n, draft) in drafts(text).into_iter().enumerate() {
        let title = if draft.title.is_empty() { untitled(n + 1) } else { draft.title };
        let mut scene = match old.iter().position(|s| s.title == title) {
            Some(at) => old.remove(at),
            None => Scene { id: story.fresh('s'), title, ..Scene::default() },
        };
        for (action, line) in draft.shots {
            let shot = match pool.iter().position(|s| s.action == action && s.line == line) {
                Some(at) => {
                    kept += 1;
                    pool.remove(at)
                }
                None => Shot {
                    id: story.fresh('h'),
                    seconds: seconds_for(&action, &line),
                    action,
                    line,
                    ..Shot::default()
                },
            };
            scene.shots.push(shot);
        }
        story.scenes.push(scene);
    }

    let speakers: Vec<String> = story
        .shots()
        .filter_map(|s| s.line.as_ref())
        .map(|l| l.who.clone())
        .filter(|who| !who.is_empty())
        .collect();
    for who in speakers {
        if story.character_named(&who).is_none() {
            story.add_character(&who, "", "");
        }
    }

    Plan {
        scenes: story.scenes.len(),
        shots: story.shots().count(),
        seconds: story.seconds(),
        kept,
        removed: before.saturating_sub(kept),
    }
}

/// What [`apply`] would do, without doing it.
pub fn plan(story: &Story, text: &str) -> Plan {
    apply(&mut story.clone(), text, &|n| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn untitled(n: usize) -> String {
        format!("Scene {n}")
    }

    const SCRIPT: &str = "场景：老周的店\n雨幕里,小店的招牌忽明忽暗。\n老周：这么晚了,姑娘要点什么?\n林夏盯着怀表。\n林夏：这块表,哪来的?\n\n场景二:巷子\n一道黑影翻过围墙。";

    #[test]
    fn scenes_and_shots_come_out_of_the_words() {
        let mut story = Story::new("t");
        let plan = apply(&mut story, SCRIPT, &untitled);
        assert_eq!((plan.scenes, plan.shots), (2, 3));
        assert_eq!(story.scenes[0].title, "老周的店");
        assert_eq!(story.scenes[1].title, "巷子");
        let first = &story.scenes[0].shots;
        assert_eq!(first[0].action, "雨幕里,小店的招牌忽明忽暗。");
        assert_eq!(first[0].line.as_ref().unwrap().who, "老周");
        assert_eq!(first[1].line.as_ref().unwrap().text, "这块表,哪来的?");
    }

    #[test]
    fn a_line_with_nothing_before_it_is_a_shot_of_its_own() {
        let mut story = Story::new("t");
        apply(&mut story, "老周：你来了。\n林夏：嗯。", &untitled);
        let shots = &story.scenes[0].shots;
        assert_eq!(shots.len(), 2);
        assert!(shots.iter().all(|s| s.action.is_empty()));
        assert_eq!(story.scenes[0].title, "Scene 1");
    }

    #[test]
    fn those_who_speak_join_the_cast_once() {
        let mut story = Story::new("t");
        apply(&mut story, SCRIPT, &untitled);
        apply(&mut story, SCRIPT, &untitled);
        let names: Vec<&str> = story.cast.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["老周", "林夏"]);
    }

    #[test]
    fn splitting_again_keeps_what_was_made_for_shots_that_did_not_change() {
        let mut story = Story::new("t");
        apply(&mut story, SCRIPT, &untitled);
        let id = story.scenes[0].shots[0].id.clone();
        story.shot_mut(&id).unwrap().seconds = 9.0;
        let edited = SCRIPT.replace("一道黑影翻过围墙。", "黑影消失在巷口。");
        let plan = apply(&mut story, &edited, &untitled);
        assert_eq!((plan.kept, plan.removed, plan.shots), (2, 1, 3));
        assert_eq!(story.scenes[0].shots[0].id, id);
        assert_eq!(story.scenes[0].shots[0].seconds, 9.0);
    }

    #[test]
    fn a_plan_tells_what_would_go_without_changing_anything() {
        let mut story = Story::new("t");
        apply(&mut story, SCRIPT, &untitled);
        let before = story.clone();
        let plan = plan(&story, "场景：老周的店\n雨幕里,小店的招牌忽明忽暗。\n老周：这么晚了,姑娘要点什么?");
        assert_eq!((plan.kept, plan.removed), (1, 2));
        assert_eq!(story, before);
    }

    #[test]
    fn english_headings_and_hash_headings_start_scenes() {
        let mut story = Story::new("t");
        apply(&mut story, "Scene 1: The shop\nRain.\n# The alley\nA shadow.", &untitled);
        let titles: Vec<&str> = story.scenes.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["The shop", "The alley"]);
    }

    #[test]
    fn a_spoken_shot_lasts_as_long_as_the_words_take() {
        let long = Some(Line { who: "a".into(), text: "字".repeat(24) });
        assert_eq!(seconds_for("x", &None), 5.0);
        assert_eq!(seconds_for("x", &long), 7.0);
        assert_eq!(seconds_for("", &Some(Line { who: "a".into(), text: "嗯".into() })), 3.0);
        assert_eq!(seconds_for("x", &Some(Line { who: "a".into(), text: "字".repeat(200) })), 15.0);
    }
}
