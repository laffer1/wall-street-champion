//! Generated tipsters. Each has a hidden reliability; every week a few of
//! them offer tips. The player can follow or ignore them.

use crate::rng::Rng;
use super::market::{Market, PendingWeek};

#[derive(Clone, Debug)]
pub struct Person {
    pub name: String,
    pub archetype: &'static str,
    /// Probability that this person's tip direction is correct.
    pub reliability: f64,
    /// Tips given / correct so far (revealed at the end).
    pub tips_given: u32,
    pub tips_correct: u32,
    pub trusted: bool,
}

#[derive(Clone, Debug)]
pub struct Tip {
    pub person: usize,
    pub symbol_idx: usize,
    pub up: bool,
    pub text: String,
    /// Filled in when the week advances.
    pub was_correct: Option<bool>,
}

const FIRST: &[&str] = &["Sal", "Vera", "Moss", "Ines", "Dutch", "Priya", "Gus", "Lena", "Otto", "Faye", "Rex", "Nia"];
const LAST: &[&str] = &["Kowalski", "Marsh", "Delgado", "Okafor", "Brandt", "Silva", "Novak", "Hale", "Fontaine", "Reyes"];
const ARCHETYPES: &[&str] = &[
    "bartender at the exchange club",
    "retired floor trader",
    "self-styled trading influencer",
    "cab driver",
    "junior analyst at a small fund",
    "old college roommate",
    "guy at the gym",
    "crypto Discord moderator",
];

pub fn generate_people(rng: &mut Rng) -> Vec<Person> {
    let mut people = Vec::new();
    let n = 8;
    for i in 0..n {
        let name = format!("{} {}", FIRST[rng.usize_below(FIRST.len())], LAST[rng.usize_below(LAST.len())]);
        people.push(Person {
            name,
            archetype: ARCHETYPES[i % ARCHETYPES.len()],
            reliability: rng.range(0.35, 0.90),
            tips_given: 0,
            tips_correct: 0,
            trusted: false,
        });
    }
    people
}

/// Generate 2-4 tips for the coming week. Tips are weighted toward the
/// biggest expected movers (insiders "know"), but each person's reliability
/// decides whether the direction they give is actually right.
pub fn generate_tips(rng: &mut Rng, people: &mut Vec<Person>, market: &Market, pending: &PendingWeek) -> Vec<Tip> {
    let mut movers: Vec<usize> = (0..market.defs.len())
        .filter(|&i| pending.rets[i].abs() > 0.02)
        .collect();
    if movers.is_empty() {
        movers = (0..market.defs.len()).collect();
    }
    movers.sort_by(|&a, &b| pending.rets[b].abs().partial_cmp(&pending.rets[a].abs()).unwrap());
    movers.truncate(6);

    let n_tips = 2 + rng.usize_below(3);
    let mut tips = Vec::new();
    let mut used_people: Vec<usize> = Vec::new();
    for _ in 0..n_tips {
        let p = loop {
            let p = rng.usize_below(people.len());
            if !used_people.contains(&p) {
                used_people.push(p);
                break p;
            }
            if used_people.len() >= people.len() {
                break usize::MAX;
            }
        };
        if p == usize::MAX {
            break;
        }
        let sym = *rng.pick(&movers);
        let actual_up = pending.rets[sym] > 0.0;
        // The person tells the truth with probability = reliability.
        let says_up = if rng.chance(people[p].reliability) { actual_up } else { !actual_up };
        let d = &market.defs[sym];
        let text = if says_up {
            format!("{} is about to rip higher. Trust me on this one.", d.symbol)
        } else {
            format!("Get out of {} NOW. It's going to tank.", d.symbol)
        };
        people[p].tips_given += 1;
        tips.push(Tip { person: p, symbol_idx: sym, up: says_up, text, was_correct: None });
    }
    tips
}

/// Reveal tip outcomes after the week's returns are known.
pub fn reveal_tips(people: &mut Vec<Person>, tips: &mut Vec<Tip>, pending: &PendingWeek) {
    for t in tips.iter_mut() {
        let actual_up = pending.rets[t.symbol_idx] > 0.0;
        let correct = if pending.rets[t.symbol_idx].abs() < 0.003 {
            false // flat week: nobody called it
        } else {
            actual_up == t.up
        };
        t.was_correct = Some(correct);
        if correct {
            people[t.person].tips_correct += 1;
        }
    }
}
