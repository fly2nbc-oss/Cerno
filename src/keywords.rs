//! Keywords across photos (CER-68): the vocabulary behind the description tab's suggestions and
//! the keyword filter's card, and what that filter picks. One keyword in any case is one word
//! ("Bodensee" = "bodensee"); the spelling the folder uses most wins. Pure, unit-tested.

use std::collections::{HashMap, HashSet};

/// A keyword as it is compared: trimmed, in lower case.
pub fn fold(keyword: &str) -> String {
    keyword.trim().to_lowercase()
}

/// One word of the vocabulary: its spelling and on how many photos it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub text: String,
    /// Photos of the open folder.
    pub folder: usize,
    /// Files of the whole index (the folder's among them).
    pub index: usize,
}

/// The keywords known: the open folder's first, the rest of the index after them – each
/// group the most used first.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Vocabulary {
    entries: Vec<Entry>,
}

#[derive(Default)]
struct Tally {
    folder: usize,
    index: usize,
    /// Each spelling with its count in the folder and in the index.
    spellings: HashMap<String, (usize, usize)>,
}

impl Vocabulary {
    /// `folder`: the keywords of each photo of the open folder; `index`: each spelling of the
    /// whole index with the number of files that have it. A photo counts once per word.
    pub fn build<'a>(
        folder: impl IntoIterator<Item = &'a [String]>,
        index: &[(String, usize)],
    ) -> Self {
        let mut tallies: HashMap<String, Tally> = HashMap::new();
        for keywords in folder {
            let mut seen = HashSet::new();
            for keyword in keywords {
                let text = keyword.trim();
                let key = fold(text);
                if key.is_empty() || !seen.insert(key.clone()) {
                    continue;
                }
                let tally = tallies.entry(key).or_default();
                tally.folder += 1;
                tally.spellings.entry(text.to_owned()).or_default().0 += 1;
            }
        }
        for (keyword, count) in index {
            let text = keyword.trim();
            let key = fold(text);
            if key.is_empty() || *count == 0 {
                continue;
            }
            let tally = tallies.entry(key).or_default();
            tally.index += count;
            tally.spellings.entry(text.to_owned()).or_default().1 += count;
        }
        let mut entries: Vec<Entry> = tallies
            .into_values()
            .filter_map(|tally| {
                // The folder's most used spelling, else the index's; ties by name, so the
                // result never depends on the hash order.
                let text = tally
                    .spellings
                    .iter()
                    .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                    .map(|(text, _)| text.clone())?;
                Some(Entry {
                    text,
                    folder: tally.folder,
                    index: tally.index.max(tally.folder),
                })
            })
            .collect();
        entries.sort_by(|a, b| {
            (b.folder > 0)
                .cmp(&(a.folder > 0))
                .then(b.folder.cmp(&a.folder))
                .then(b.index.cmp(&a.index))
                .then_with(|| fold(&a.text).cmp(&fold(&b.text)))
                .then_with(|| a.text.cmp(&b.text))
        });
        Self { entries }
    }

    #[cfg(test)]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The open folder's keywords, the most used first.
    pub fn in_folder(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|entry| entry.folder > 0)
    }

    /// How a typed keyword is written: as the vocabulary spells it when it knows the word,
    /// else as typed (trimmed).
    pub fn spelling(&self, typed: &str) -> String {
        let key = fold(typed);
        self.entries
            .iter()
            .find(|entry| fold(&entry.text) == key)
            .map_or_else(|| typed.trim().to_owned(), |entry| entry.text.clone())
    }

    /// At most `limit` words for what is being typed: those that start with it, or one of whose
    /// words does ("see" finds "Lake See", not "Bodensee"), the folder's first and the most
    /// used first – without the ones the photo has already.
    pub fn suggest(&self, typed: &str, have: &[String], limit: usize) -> Vec<&Entry> {
        let typed = fold(typed);
        if typed.is_empty() {
            return Vec::new();
        }
        let have: HashSet<String> = have.iter().map(|k| fold(k)).collect();
        self.entries
            .iter()
            .filter(|entry| {
                let key = fold(&entry.text);
                !have.contains(&key) && starts_a_word(&key, &typed)
            })
            .take(limit)
            .collect()
    }
}

/// `text` starts with `typed`, or one of its words does (after a space, a hyphen, a slash …).
fn starts_a_word(text: &str, typed: &str) -> bool {
    text.starts_with(typed)
        || text
            .match_indices(|c: char| !c.is_alphanumeric())
            .any(|(at, sep)| text[at + sep.len()..].starts_with(typed))
}

/// What the keyword filter shows: the photos with one of these words – or, with `none`, the
/// photos without any keyword. One of them is enough (OR); with the other filters it is AND.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pick {
    /// As shown in the chip, in the order chosen.
    pub keywords: Vec<String>,
    pub none: bool,
}

impl Pick {
    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty() && !self.none
    }

    /// Whether a photo with these keywords passes.
    pub fn accepts(&self, keywords: &[String]) -> bool {
        if self.none && keywords.iter().all(|k| k.trim().is_empty()) {
            return true;
        }
        keywords.iter().any(|keyword| {
            let key = fold(keyword);
            self.keywords.iter().any(|picked| fold(picked) == key)
        })
    }

    /// Whether the word is picked.
    pub fn contains(&self, keyword: &str) -> bool {
        let key = fold(keyword);
        self.keywords.iter().any(|picked| fold(picked) == key)
    }

    /// The word picked, or no longer.
    pub fn toggle(&mut self, keyword: &str) {
        let key = fold(keyword);
        if let Some(at) = self.keywords.iter().position(|k| fold(k) == key) {
            self.keywords.remove(at);
        } else {
            self.keywords.push(keyword.trim().to_owned());
        }
    }
}

/// The index stores a file's keywords as one text, a line each (a keyword never holds a line
/// break: the writer makes it a space).
pub fn join(keywords: &[String]) -> String {
    keywords
        .iter()
        .map(|k| clean(k))
        .filter(|k| !k.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn split(stored: &str) -> Vec<String> {
    stored
        .split('\n')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A keyword as it is written: one line, trimmed.
pub fn clean(keyword: &str) -> String {
    keyword.replace(['\r', '\n'], " ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn vocabulary() -> Vocabulary {
        let photos = [
            words(&["Bodensee", "Familie"]),
            words(&["bodensee", "Strand"]),
            words(&["Bodensee", "Lake Constance"]),
            words(&["Familie", "familie"]),
        ];
        let index = [
            ("Bodensee".to_owned(), 5),
            ("Berge".to_owned(), 40),
            ("Bodensee-Radweg".to_owned(), 2),
            ("Familie".to_owned(), 2),
        ];
        Vocabulary::build(photos.iter().map(Vec::as_slice), &index)
    }

    /// One word in any case; the folder's most used spelling; a photo counts once per word.
    #[test]
    fn one_word_in_any_case() {
        let v = vocabulary();
        let folder: Vec<(&str, usize)> =
            v.in_folder().map(|e| (e.text.as_str(), e.folder)).collect();
        assert_eq!(
            folder,
            [
                ("Bodensee", 3),
                ("Familie", 2),
                ("Lake Constance", 1),
                ("Strand", 1)
            ]
        );
        // The index's words come after the folder's, the most used first.
        let rest: Vec<&str> = v
            .entries()
            .iter()
            .filter(|e| e.folder == 0)
            .map(|e| e.text.as_str())
            .collect();
        assert_eq!(rest, ["Berge", "Bodensee-Radweg"]);
    }

    #[test]
    fn suggestions_start_a_word_and_leave_out_what_the_photo_has() {
        let v = vocabulary();
        let texts = |typed: &str, have: &[&str]| -> Vec<String> {
            v.suggest(typed, &words(have), 6)
                .into_iter()
                .map(|e| e.text.clone())
                .collect()
        };
        assert_eq!(texts("bo", &[]), ["Bodensee", "Bodensee-Radweg"]);
        assert_eq!(texts("BO", &["bodensee"]), ["Bodensee-Radweg"]);
        // A later word of the keyword counts; the middle of a word doesn't.
        assert_eq!(texts("rad", &[]), ["Bodensee-Radweg"]);
        assert_eq!(texts("const", &[]), ["Lake Constance"]);
        assert!(texts("densee", &[]).is_empty());
        assert!(texts("  ", &[]).is_empty());
        assert_eq!(v.suggest("b", &[], 1).len(), 1, "the limit holds");
    }

    /// A word the vocabulary knows takes its spelling; a new one stays as typed.
    #[test]
    fn a_known_word_keeps_its_spelling() {
        let v = vocabulary();
        assert_eq!(v.spelling(" bodensee "), "Bodensee");
        assert_eq!(v.spelling("berge"), "Berge");
        assert_eq!(v.spelling(" Neu "), "Neu");
    }

    #[test]
    fn a_pick_takes_any_of_its_words_or_none() {
        let mut pick = Pick::default();
        assert!(pick.is_empty());
        pick.toggle("Familie");
        pick.toggle("Strand");
        assert!(pick.accepts(&words(&["familie", "Berge"])));
        assert!(!pick.accepts(&words(&["Berge"])));
        assert!(!pick.accepts(&[]), "without keywords only with `none`");
        pick.toggle("FAMILIE");
        assert_eq!(pick.keywords, ["Strand"]);
        pick.none = true;
        assert!(pick.accepts(&[]));
        assert!(pick.accepts(&words(&["strand"])));
        assert!(!pick.accepts(&words(&["Berge"])));
        assert!(pick.contains("STRAND"));
    }

    #[test]
    fn stored_one_per_line() {
        let stored = join(&words(&["Straße", " zwei\nZeilen ", ""]));
        assert_eq!(stored, "Straße\nzwei Zeilen");
        assert_eq!(split(&stored), ["Straße", "zwei Zeilen"]);
        assert!(split("").is_empty());
    }
}
