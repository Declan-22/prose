//! Heuristic prose checks. These intentionally return ranges, keeping the UI
//! independent from the check implementations and making them replaceable.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { Filler, PassiveVoice, LoosePronouns, RepetitionRadar, CommonlyMisused, LexicalIllusions, Dialogue, SentenceRhythm }

impl Kind {
    pub const ALL: [Kind; 8] = [Self::Filler, Self::PassiveVoice, Self::LoosePronouns, Self::RepetitionRadar, Self::CommonlyMisused, Self::LexicalIllusions, Self::Dialogue, Self::SentenceRhythm];
    pub fn label(self) -> &'static str { match self { Self::Filler=>"Filler words", Self::PassiveVoice=>"Passive voice", Self::LoosePronouns=>"Loose pronouns", Self::RepetitionRadar=>"Repetition radar", Self::CommonlyMisused=>"Commonly misused", Self::LexicalIllusions=>"Lexical illusions", Self::Dialogue=>"Dialogue", Self::SentenceRhythm=>"Sentence rhythm" } }
    pub fn index(self) -> usize { Self::ALL.iter().position(|k| *k == self).unwrap() }
}

#[derive(Clone, Debug)]
pub struct Span { pub start: usize, pub end: usize, pub kind: Kind }

fn token_ranges(text: &str) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_alphanumeric() || c == '\'' { if start.is_none() { start = Some(i); } }
        else if let Some(s) = start.take() { out.push((s, i, text[s..i].to_ascii_lowercase())); }
    }
    if let Some(s) = start { out.push((s, text.len(), text[s..].to_ascii_lowercase())); }
    out
}

fn add_word(out: &mut Vec<Span>, text: &str, word: &str, kind: Kind) {
    let needle = word.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = text[from..].to_ascii_lowercase().find(&needle) { let s = from + rel; let e = s + word.len(); out.push(Span { start: s, end: e, kind }); from = e; }
}

pub fn find(text: &str, enabled: &[bool; 8]) -> Vec<Span> {
    let toks = token_ranges(text);
    let mut out = Vec::new();
    let filler = ["just", "really", "very", "actually", "perhaps", "maybe", "that", "quite", "somewhat"];
    let pronouns = ["it", "this", "that", "these", "those", "they", "them", "he", "she"];
    let misused = ["your", "you're", "its", "it's", "affect", "effect", "then", "than", "there", "their", "they're"];
    for (s, e, w) in &toks {
        let kind = if filler.contains(&w.as_str()) { Some(Kind::Filler) } else if pronouns.contains(&w.as_str()) { Some(Kind::LoosePronouns) } else if misused.contains(&w.as_str()) { Some(Kind::CommonlyMisused) } else { None };
        if let Some(k) = kind { if enabled[k.index()] { out.push(Span { start: *s, end: *e, kind: k }); } }
    }
    if enabled[Kind::PassiveVoice.index()] { for (s, e, w) in &toks { if ["was", "were", "is", "are", "been", "be"].contains(&w.as_str()) { if let Some((_, ne, nw)) = toks.iter().find(|(ns, _, _)| *ns > *e && *ns - *e < 12) { if nw.ends_with("ed") || nw.ends_with("en") { out.push(Span { start: *s, end: *ne, kind: Kind::PassiveVoice }); } } } } }
    if enabled[Kind::RepetitionRadar.index()] { for pair in toks.windows(2) { if pair[0].2 == pair[1].2 && pair[0].2.len() > 2 { out.push(Span { start: pair[0].0, end: pair[1].1, kind: Kind::RepetitionRadar }); } } }
    if enabled[Kind::Dialogue.index()] { let mut quote = None; for (i, c) in text.char_indices() { if c == '"' { if let Some(s) = quote.take() { out.push(Span { start: s, end: i + 1, kind: Kind::Dialogue }); } else { quote = Some(i); } } } }
    if enabled[Kind::SentenceRhythm.index()] { let mut s = 0; for (i, c) in text.char_indices() { if ".!?".contains(c) { let e = i + c.len_utf8(); if e.saturating_sub(s) > 120 || e.saturating_sub(s) < 25 { out.push(Span { start: s, end: e, kind: Kind::SentenceRhythm }); } s = e; } } }
    if enabled[Kind::LexicalIllusions.index()] { for word in ["unexcepted", "irregardless", "could of", "should of", "alot"] { add_word(&mut out, text, word, Kind::LexicalIllusions); } }
    out
}
