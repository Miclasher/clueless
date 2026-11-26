//! A cheap local check for "was something just asked?", so that automatic
//! answering does not spend subscription usage on every sentence.

const STARTERS: &[&str] = &[
    // English
    "what",
    "what's",
    "why",
    "how",
    "when",
    "where",
    "who",
    "which",
    "can you",
    "could you",
    "would you",
    "will you",
    "do you",
    "did you",
    "have you",
    "are you",
    "is there",
    "is it",
    "explain",
    "describe",
    "define",
    "name",
    "give me",
    "give us",
    "compare",
    "walk me",
    "talk about",
    "tell",
    // Ukrainian
    "що",
    "чому",
    "як",
    "коли",
    "де",
    "хто",
    "який",
    "яка",
    "яке",
    "які",
    "яку",
    "чи",
    "скільки",
    "навіщо",
    "розкажіть",
    "розкажи",
    "поясніть",
    "поясни",
    "опишіть",
    "опиши",
    "назвіть",
    "назви",
    "порівняйте",
    // Russian
    "что",
    "почему",
    "как",
    "когда",
    "где",
    "кто",
    "какой",
    "какая",
    "какое",
    "какие",
    "какую",
    "сколько",
    "зачем",
    "расскажите",
    "расскажи",
    "объясните",
    "объясни",
    "опишите",
    "назовите",
    "сравните",
];

const ANYWHERE: &[&str] = &[
    "can you",
    "could you",
    "would you",
    "tell me",
    "tell us",
    "walk me through",
    "i'd like to hear",
    "можете",
    "можеш",
    "скажіть",
    "підкажіть",
    "скажите",
    "подскажите",
    "розкажіть",
    "поясніть",
    "расскажите",
    "объясните",
    "how about",
    "what about",
    "i'm curious",
    "i am curious",
    "i wonder",
    "i'd love to hear",
    "i'd love to know",
    "i'd like to know",
    "i would like to know",
    "let's talk about",
    "а як щодо",
    "що щодо",
    "цікаво почути",
    "хотілося б почути",
    "как насчет",
    "как насчёт",
    "интересно услышать",
    "хотелось бы услышать",
];

/// Question openings that also count after a comma: "Given that, how would
/// you ..." Single words such as "which" would match ordinary clauses.
const AFTER_COMMA: &[&str] = &[
    "how would",
    "how do",
    "how did",
    "how can",
    "how are",
    "what is",
    "what's",
    "what are",
    "what would",
    "why did",
    "why do",
    "why would",
    "can you",
    "could you",
    "would you",
    "do you",
    "did you",
    "have you",
    "are you",
];

/// A line ending in one of these words stops in the middle of a sentence.
const DANGLING: &[&str] = &[
    "a",
    "an",
    "the",
    "and",
    "or",
    "but",
    "so",
    "to",
    "of",
    "with",
    "for",
    "if",
    "that",
    "because",
    "about",
    "in",
    "on",
    "your",
    "our",
    "і",
    "й",
    "та",
    "або",
    "але",
    "що",
    "щоб",
    "з",
    "в",
    "у",
    "на",
    "про",
    "для",
    "якщо",
    "тому",
    "и",
    "или",
    "но",
    "что",
    "чтобы",
    "с",
    "о",
    "если",
    "потому",
];

const FILLERS: &[&str] = &[
    "so",
    "and",
    "ok",
    "okay",
    "well",
    "now",
    "then",
    "alright",
    "um",
    "uh",
    "please",
    "а",
    "і",
    "и",
    "то",
    "ну",
    "отже",
    "добре",
    "тоді",
    "так",
    "хорошо",
    "тогда",
    "итак",
    "будь",
    "ласка",
    "пожалуйста",
];

/// "Please, when ..." is a request even without a question mark.
const POLITE: &[&str] = &["please", "будь ласка", "пожалуйста"];

/// Is a question being asked by `line`, possibly one that began on the
/// `previous` line before a pause?
pub fn asks(previous: &str, line: &str) -> bool {
    if looks_like_question(line) {
        return true;
    }
    if previous.is_empty() || looks_like_question(previous) {
        return false;
    }
    let start = previous.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '.' | ',' | '…' | '-'));
    looks_like_question(&format!("{start} {line}"))
}

/// Does the line sound as if more is about to follow?
pub fn looks_unfinished(line: &str) -> bool {
    let line = line.trim_end();
    if line.ends_with([',', ':', '-', '—', '…']) || line.ends_with("...") {
        return true;
    }
    if line.ends_with(['?', '!']) {
        return false;
    }
    line.trim_end_matches('.')
        .split_whitespace()
        .next_back()
        .is_some_and(|w| DANGLING.contains(&w.to_lowercase().as_str()))
}

pub fn looks_like_question(text: &str) -> bool {
    if text.contains('?') {
        return true;
    }
    let lower = text.to_lowercase().replace('’', "'");
    lower.split(['.', '!', ';', '\n']).any(|sentence| {
        let words: Vec<&str> = sentence
            .split(|c: char| !(c.is_alphanumeric() || c == '\''))
            .filter(|w| !w.is_empty())
            .skip_while(|w| FILLERS.contains(w))
            .collect();
        if words.len() < 2 {
            return false;
        }
        let joined = format!(" {} ", words.join(" "));
        let starts = |text: &str| STARTERS.iter().any(|s| text.starts_with(&format!(" {s} ")));
        starts(&joined)
            || ANYWHERE.iter().any(|s| joined.contains(&format!(" {s} ")))
            || POLITE.iter().any(|p| {
                joined
                    .split_once(&format!(" {p} "))
                    .is_some_and(|(_, rest)| starts(&format!(" {rest}")))
            })
            || sentence.split(',').skip(1).any(opens_a_question)
    })
}

fn opens_a_question(clause: &str) -> bool {
    let words: Vec<&str> = clause
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .skip_while(|w| FILLERS.contains(w))
        .collect();
    let joined = format!("{} ", words.join(" "));
    words.len() >= 2 && AFTER_COMMA.iter().any(|s| joined.starts_with(&format!("{s} ")))
}

#[cfg(test)]
mod tests {
    use super::{asks, looks_like_question, looks_unfinished};

    #[test]
    fn detects_questions() {
        for q in [
            "What is a borrow checker",
            "Okay, so how would you design a cache.",
            "Great. Tell me about your last project.",
            "I'd like you to, if you can you know, tell me why",
            "Отже, розкажіть про ваш досвід.",
            "Оскажіть, будь ласка, коли запланований запуст проекту.",
            "Скажіть, а які у вас є зауваження",
            "Почему вы выбрали Rust",
            "Is that right?",
            "Given the traffic you described, how would you scale it.",
            "I'm curious about your experience with databases.",
            "And what about testing.",
        ] {
            assert!(looks_like_question(q), "{q}");
        }
    }

    #[test]
    fn ignores_statements() {
        for s in [
            "Thank you.",
            "We use Rust at our company.",
            "Добре, дякую.",
            "Дякую, будь ласка.",
            "Okay.",
            "How.",
            "We rewrote it, which helped a lot.",
        ] {
            assert!(!looks_like_question(s), "{s}");
        }
    }

    #[test]
    fn joins_questions_split_by_a_pause() {
        assert!(asks("Could.", "You walk us through it."));
        assert!(asks("So can", "you start."));
        // An unrelated line after a question is not a new question.
        assert!(!asks("What is Rust?", "Take your time."));
        assert!(!asks("We use Rust.", "Thanks."));
    }

    #[test]
    fn detects_unfinished_lines() {
        for s in [
            "Tell me about the",
            "So, thinking about it,",
            "Розкажіть про...",
            "Як ви працюєте з",
        ] {
            assert!(looks_unfinished(s), "{s}");
        }
        for s in ["What is it about?", "We use Rust.", "Why Rust"] {
            assert!(!looks_unfinished(s), "{s}");
        }
    }
}
