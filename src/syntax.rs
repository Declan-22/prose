//! Word-Smith-style offline part-of-speech tagger: lexicon, suffix, context.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind { Nouns, Verbs, Adverbs, Adjectives, Conjunctions }
impl Kind {
    pub const ALL: [Kind; 5] = [Self::Nouns, Self::Verbs, Self::Adverbs, Self::Adjectives, Self::Conjunctions];
    pub fn label(self) -> &'static str { match self { Self::Nouns=>"Nouns", Self::Verbs=>"Verbs", Self::Adverbs=>"Adverbs", Self::Adjectives=>"Adjectives", Self::Conjunctions=>"Conjunctions" } }
    pub fn index(self) -> usize { match self { Self::Nouns=>0, Self::Verbs=>1, Self::Adverbs=>2, Self::Adjectives=>3, Self::Conjunctions=>4 } }
}
#[derive(Clone, Debug)]
pub struct Span { pub start: usize, pub end: usize, pub kind: Kind }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag { Noun, Verb, Adverb, Adjective, Conjunction, Determiner, Pronoun, Preposition, Other }
#[derive(Clone, Debug)]
struct Token { start: usize, end: usize, tag: Tag }

fn lexicon(w: &str) -> Option<Tag> {
    Some(match w {
        // Function words and common -ing/-ly exceptions must be known before
        // suffix rules run. Otherwise "everything" becomes a verb simply
        // because it ends in -ing, and "otherwise" can be misread by a
        // context rule.
        "there"|"here"|"where"|"when"|"why"|"how"|"not"|"no"|"yes"|"as"|"than"|"then"|"otherwise"|"everything"|"something"|"anything"|"nothing"|"morning"|"evening"|"being"|"going" => Tag::Other,
        "a"|"an"|"the"|"this"|"that"|"these"|"those"|"each"|"every"|"some"|"any"|"my"|"your"|"his"|"her"|"our"|"their" => Tag::Determiner,
        "i"|"me"|"we"|"us"|"you"|"he"|"him"|"she"|"it"|"they"|"them"|"who"|"whom"|"which" => Tag::Pronoun,
        "in"|"on"|"at"|"by"|"from"|"with"|"about"|"against"|"between"|"into"|"through"|"during"|"without"|"before"|"after"|"under"|"over" => Tag::Preposition,
        "and"|"or"|"but"|"nor"|"for"|"yet"|"so"|"although"|"because"|"while"|"if"|"unless"|"though"|"whereas" => Tag::Conjunction,
        "be"|"is"|"am"|"are"|"was"|"were"|"been"|"have"|"has"|"had"|"do"|"does"|"did"|"can"|"could"|"will"|"would"|"shall"|"should"|"may"|"might"|"must" => Tag::Verb,
        "make"|"made"|"go"|"goes"|"went"|"gone"|"say"|"says"|"said"|"see"|"sees"|"saw"|"seen"|"know"|"knows"|"knew"|"think"|"thinks"|"thought"|"write"|"writes"|"wrote"|"run"|"runs"|"ran" => Tag::Verb,
        "good"|"bad"|"new"|"old"|"young"|"large"|"small"|"quick"|"slow"|"happy"|"sad"|"important"|"different"|"difficult" => Tag::Adjective,
        "person"|"people"|"time"|"day"|"world"|"life"|"way"|"man"|"woman"|"child"|"place"|"thing"|"house"|"story"|"book"|"work"|"meeting" => Tag::Noun,
        _ => return None,
    })
}
fn suffix(w: &str) -> Tag {
    if w.ends_with("ly") && !matches!(w, "family"|"reply"|"early") { Tag::Adverb }
    else if w.ends_with("ing") || w.ends_with("ed") || w.ends_with("ize") || w.ends_with("ise") { Tag::Verb }
    else if ["tion","ment","ness","ity","ship","dom"].iter().any(|s| w.ends_with(s)) { Tag::Noun }
    else if ["ous","ful","less","able","ible","ive","al","ic"].iter().any(|s| w.ends_with(s)) { Tag::Adjective }
    else { Tag::Other }
}
fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new(); let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_alphanumeric() || c == '\'' { if start.is_none() { start = Some(i); } }
        else if let Some(s) = start.take() { let word = text[s..i].to_ascii_lowercase(); let tag = lexicon(&word).unwrap_or_else(|| suffix(&word)); out.push(Token { start:s, end:i, tag }); }
    }
    if let Some(s) = start { let word = text[s..].to_ascii_lowercase(); let tag = lexicon(&word).unwrap_or_else(|| suffix(&word)); out.push(Token { start:s, end:text.len(), tag }); }
    out
}
fn context_pass(ts: &mut [Token]) {
    for i in 0..ts.len() {
        let previous = i.checked_sub(1).map(|n| ts[n].tag); let next = ts.get(i+1).map(|t| t.tag);
        // Context is intentionally corrective, never generative: it may
        // change a known noun in an adjective slot, but it must not invent a
        // noun or verb tag for an unknown function word.
        if previous == Some(Tag::Determiner) && next == Some(Tag::Noun) && ts[i].tag == Tag::Noun {
            ts[i].tag = Tag::Adjective;
        }
    }
}
fn kind(tag: Tag) -> Option<Kind> { match tag { Tag::Noun=>Some(Kind::Nouns), Tag::Verb=>Some(Kind::Verbs), Tag::Adverb=>Some(Kind::Adverbs), Tag::Adjective=>Some(Kind::Adjectives), Tag::Conjunction=>Some(Kind::Conjunctions), _=>None } }
pub fn find(text: &str, enabled: &[bool; 5]) -> Vec<Span> {
    let mut ts = tokenize(text); context_pass(&mut ts);
    ts.into_iter().filter_map(|t| kind(t.tag).filter(|k| enabled[k.index()]).map(|k| Span { start:t.start, end:t.end, kind:k })).collect()
}
