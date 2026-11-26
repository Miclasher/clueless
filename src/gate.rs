//! Decides when to ask Claude to answer. Pauses alone never trigger anything:
//! a question has to be heard, and then the other side has to stop talking.

use std::time::{Duration, Instant};

use crate::question;

/// Silence after a question before it is answered.
const PAUSE: Duration = Duration::from_millis(2500);
/// Silence after a line that stops in the middle of a sentence.
const UNFINISHED_PAUSE: Duration = Duration::from_secs(5);
/// "Speech" that produces no new words for this long is background noise.
const STALL: Duration = Duration::from_secs(6);
/// Answer after this long even if the other side never stops.
const MAX_WAIT: Duration = Duration::from_secs(60);
/// Speech resuming this soon after a request may belong to the same question.
const CONTINUATION: Duration = Duration::from_secs(10);
/// The same, after Claude found the question missing or unfinished.
const AFTER_WAIT: Duration = Duration::from_secs(60);
/// Extra requests allowed for continuations of one question.
const FOLLOW_UPS: u8 = 2;
/// How long voice detection waits before reporting silence.
const HANGOVER: Duration = Duration::from_millis(750);
/// How far recognised words typically trail the voice.
const TEXT_LAG: Duration = Duration::from_millis(700);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// A new question was heard.
    Question,
    /// More was said after a request, possibly the rest of the question.
    FollowUp,
}

/// What has been heard from the other side, as far as timing is concerned.
#[derive(Default)]
pub struct Gate {
    voice: bool,
    voice_started: Option<Instant>,
    voice_ended: Option<Instant>,
    partial: String,
    /// When recognised words last changed.
    text_at: Option<Instant>,
    previous: String,
    /// A request that is waiting for silence, and since when.
    asked: Option<(Instant, Reason)>,
    follow_until: Option<Instant>,
    follow_ups: u8,
    /// Speech resumed soon enough to be a continuation.
    continuing: bool,
    /// The user asked for an answer before there was a question.
    armed: bool,
}

impl Gate {
    pub fn speaking(&self) -> bool {
        self.voice || !self.partial.is_empty()
    }

    /// Words heard but not yet part of a finished line.
    pub fn partial_text(&self) -> &str {
        &self.partial
    }

    pub fn voice(&mut self, now: Instant, active: bool) {
        if active {
            if !self.voice {
                self.voice_started = Some(now);
            }
            self.resumed(now);
        } else if self.voice {
            self.voice_ended = Some(now.checked_sub(HANGOVER).unwrap_or(now));
        }
        self.voice = active;
    }

    pub fn partial(&mut self, now: Instant, text: &str) {
        if !text.is_empty() && text != self.partial {
            self.text_at = Some(now);
            self.resumed(now);
        }
        self.partial = text.to_string();
    }

    /// A finished line from the other side.
    pub fn line(&mut self, now: Instant, text: &str, auto: bool) {
        self.resumed(now);
        self.text_at = Some(now);
        let question = question::asks(&self.previous, text);
        self.previous = text.to_string();
        let continuing = std::mem::take(&mut self.continuing);
        if !auto && !self.armed {
            return;
        }
        if question {
            let since = self.asked.map_or(now, |a| a.0);
            self.asked = Some((since, Reason::Question));
            self.follow_ups = FOLLOW_UPS;
        } else if continuing && self.asked.is_none() && self.follow_ups > 0 {
            self.follow_ups -= 1;
            self.asked = Some((now, Reason::FollowUp));
        }
    }

    fn resumed(&mut self, now: Instant) {
        if self.follow_until.is_some_and(|until| now <= until) {
            self.continuing = true;
        }
    }

    /// The user is talking, so the other side's question is over.
    pub fn user_spoke(&mut self) {
        self.follow_until = None;
        self.continuing = false;
        if matches!(self.asked, Some((_, Reason::FollowUp))) {
            self.asked = None;
        }
    }

    /// Returns why an answer is due now, once.
    pub fn poll(&mut self, now: Instant) -> Option<Reason> {
        let (since, reason) = self.asked?;
        let quiet_since = |t: Option<Instant>| t.map(|t| now.saturating_duration_since(t));
        let ready = if now.saturating_duration_since(since) >= MAX_WAIT {
            true
        } else if self.speaking() {
            // Someone who has only just started again has no words to show yet.
            quiet_since(self.text_at.max(self.voice_started)).is_some_and(|q| q >= STALL)
        } else {
            let pause = if question::looks_unfinished(&self.previous) {
                UNFINISHED_PAUSE
            } else {
                PAUSE
            };
            let words_ended = self.text_at.and_then(|t| t.checked_sub(TEXT_LAG));
            quiet_since(self.voice_ended.max(words_ended)).is_none_or(|q| q >= pause)
        };
        if !ready {
            return None;
        }
        self.asked = None;
        Some(reason)
    }

    /// A request was sent. `manual` means the user asked for it.
    pub fn sent(&mut self, now: Instant, manual: bool) {
        self.asked = None;
        self.armed = false;
        self.continuing = false;
        self.follow_until = Some(now + CONTINUATION);
        if manual {
            self.follow_ups = FOLLOW_UPS;
        }
    }

    /// Claude found no complete question, so keep listening for it.
    pub fn waited(&mut self, now: Instant, manual: bool) {
        self.follow_until = Some(now + AFTER_WAIT);
        if manual {
            self.armed = true;
        }
    }

    /// Answer the next question even if automatic answering is off.
    pub fn arm(&mut self) {
        self.armed = true;
    }

    /// Forgets any pending or expected request.
    pub fn cancel(&mut self) {
        self.asked = None;
        self.armed = false;
        self.continuing = false;
        self.follow_until = None;
    }

    /// The capture stopped or restarted.
    pub fn silence(&mut self) {
        self.voice = false;
        self.partial.clear();
    }

    pub fn reset(&mut self) {
        *self = Gate::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Clock(Instant);

    impl Clock {
        /// Slightly negative times are allowed.
        fn at(&self, seconds: f64) -> Instant {
            self.0 + Duration::from_secs_f64(100.0 + seconds)
        }
    }

    fn clock() -> Clock {
        Clock(Instant::now())
    }

    /// Someone says `text`, finishing at second `end`.
    fn say(g: &mut Gate, c: &Clock, end: f64, text: &str) {
        g.voice(c.at(end - 1.0), true);
        g.partial(c.at(end - 0.5), text);
        g.partial(c.at(end + 0.5), "");
        g.line(c.at(end + 0.5), text, true);
        g.voice(c.at(end + 0.75), false);
    }

    #[test]
    fn answers_after_a_pause() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "What is a borrow checker?");
        assert_eq!(g.poll(c.at(2.0)), None);
        assert_eq!(g.poll(c.at(2.6)), Some(Reason::Question));
        assert_eq!(g.poll(c.at(9.0)), None);
    }

    #[test]
    fn silence_without_a_question_never_answers() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "We use Rust at our company.");
        say(&mut g, &c, 20.0, "The team is in Lviv.");
        for t in [25.0, 60.0, 600.0] {
            assert_eq!(g.poll(c.at(t)), None);
        }
        say(&mut g, &c, 700.0, "How would you test it?");
        assert_eq!(g.poll(c.at(703.0)), Some(Reason::Question));
    }

    #[test]
    fn rides_through_pauses() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "How would you design a cache?");
        // They continue after two seconds.
        g.voice(c.at(2.0), true);
        assert_eq!(g.poll(c.at(2.6)), None);
        say(&mut g, &c, 5.0, "One that stays consistent across servers.");
        assert_eq!(g.poll(c.at(6.0)), None);
        assert_eq!(g.poll(c.at(7.6)), Some(Reason::Question));
    }

    #[test]
    fn waits_longer_for_unfinished_sentences() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "Can you tell me about the");
        assert_eq!(g.poll(c.at(4.0)), None);
        assert_eq!(g.poll(c.at(5.1)), Some(Reason::Question));
    }

    #[test]
    fn follows_up_when_the_question_continues() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "How would you design a cache?");
        assert!(g.poll(c.at(3.0)).is_some());
        g.sent(c.at(3.0), false);
        say(&mut g, &c, 8.0, "It has to be consistent across servers.");
        assert_eq!(g.poll(c.at(11.0)), Some(Reason::FollowUp));
        g.sent(c.at(11.0), false);
        say(&mut g, &c, 15.0, "And cheap to run.");
        assert_eq!(g.poll(c.at(18.0)), Some(Reason::FollowUp));
        g.sent(c.at(18.0), false);
        // The allowance for one question is used up.
        say(&mut g, &c, 22.0, "Take your time.");
        assert_eq!(g.poll(c.at(40.0)), None);
    }

    #[test]
    fn long_continuation_counts_from_when_it_started() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "How would you design a cache?");
        assert!(g.poll(c.at(3.0)).is_some());
        g.sent(c.at(3.0), false);
        g.voice(c.at(9.0), true);
        g.partial(c.at(29.0), "a very long addition");
        g.partial(c.at(30.0), "");
        g.line(c.at(30.0), "A very long addition.", true);
        g.voice(c.at(30.2), false);
        assert_eq!(g.poll(c.at(33.0)), Some(Reason::FollowUp));
    }

    #[test]
    fn later_talk_is_not_a_continuation() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "What is Rust?");
        assert!(g.poll(c.at(3.0)).is_some());
        g.sent(c.at(3.0), false);
        say(&mut g, &c, 30.0, "Great, thank you.");
        assert_eq!(g.poll(c.at(90.0)), None);
    }

    #[test]
    fn no_follow_up_once_the_user_answers() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "What is Rust?");
        assert!(g.poll(c.at(3.0)).is_some());
        g.sent(c.at(3.0), false);
        g.user_spoke();
        say(&mut g, &c, 6.0, "Right.");
        assert_eq!(g.poll(c.at(20.0)), None);
    }

    #[test]
    fn keeps_waiting_after_an_unfinished_question() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "So how would you");
        assert!(g.poll(c.at(6.0)).is_some());
        g.sent(c.at(6.0), false);
        g.waited(c.at(8.0), false);
        // The rest arrives long afterwards.
        say(&mut g, &c, 45.0, "Scale this service.");
        assert_eq!(g.poll(c.at(48.0)), Some(Reason::FollowUp));
    }

    #[test]
    fn armed_by_the_user_without_automatic_answering() {
        let (mut g, c) = (Gate::default(), clock());
        g.line(c.at(0.0), "What is Rust?", false);
        assert_eq!(g.poll(c.at(10.0)), None);
        g.arm();
        g.line(c.at(11.0), "Hello there.", false);
        assert_eq!(g.poll(c.at(20.0)), None);
        g.line(c.at(21.0), "Why do you like it?", false);
        assert_eq!(g.poll(c.at(25.0)), Some(Reason::Question));
        g.sent(c.at(25.0), false);
        g.line(c.at(40.0), "And why now?", false);
        assert_eq!(g.poll(c.at(50.0)), None);
    }

    #[test]
    fn noise_and_endless_talk_do_not_block_forever() {
        let (mut g, c) = (Gate::default(), clock());
        // Voice detection stays on, but no further words arrive.
        g.voice(c.at(0.0), true);
        g.line(c.at(1.0), "What is Rust?", true);
        assert_eq!(g.poll(c.at(5.0)), None);
        assert_eq!(g.poll(c.at(7.1)), Some(Reason::Question));
        // Words keep arriving without any pause.
        g.line(c.at(10.0), "What is Go?", true);
        for t in 11..70 {
            g.partial(c.at(t as f64), &format!("word {t}"));
            assert_eq!(g.poll(c.at(t as f64)), None);
        }
        assert_eq!(g.poll(c.at(70.0)), Some(Reason::Question));
    }

    #[test]
    fn speech_that_just_resumed_is_not_noise() {
        let (mut g, c) = (Gate::default(), clock());
        g.line(c.at(0.0), "What is Rust?", true);
        g.voice(c.at(20.0), true);
        assert_eq!(g.poll(c.at(20.1)), None);
        assert_eq!(g.poll(c.at(26.1)), Some(Reason::Question));
    }

    #[test]
    fn cancel_forgets_the_question() {
        let (mut g, c) = (Gate::default(), clock());
        say(&mut g, &c, 0.0, "What is Rust?");
        g.cancel();
        assert_eq!(g.poll(c.at(100.0)), None);
    }
}
