// FSRS v4 (Free Spaced Repetition Scheduler) algorithm implementation
// Reference: https://github.com/open-spaced-repetition/fsrs4anki
//
// Default weights calibrated by FSRS team. Override via `FSRSWeights` for
// per-user optimization.

use serde::{Deserialize, Serialize};

/// Rating given by user after a review.
/// 1 = Again (forgot), 2 = Hard, 3 = Good, 4 = Easy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

impl Rating {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Rating::Again),
            2 => Some(Rating::Hard),
            3 => Some(Rating::Good),
            4 => Some(Rating::Easy),
            _ => None,
        }
    }
}

/// State of a card in the learning pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CardState {
    New,
    Learning,
    Review,
    Relearning,
}

/// FSRS model weights (v4 defaults).
/// Index: 0..=17 corresponding to w0..w17 in the algorithm spec.
#[derive(Debug, Clone)]
pub struct FSRSWeights {
    pub w: [f64; 17],
}

impl Default for FSRSWeights {
    fn default() -> Self {
        // FSRS-4.5 default weights (latest published defaults)
        Self {
            w: [
                0.4072, 1.1829, 3.1262, 15.4722, 7.2102, 0.5316, 1.0651, 0.0234, 1.616, 0.1544,
                1.0824, 1.9813, 0.0953, 0.2975, 2.2042, 0.2407, 2.9466,
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSchedule {
    pub difficulty: f64,
    pub stability: f64,
    pub interval_days: f64,
    pub reps: i64,
    pub lapses: i64,
    pub last_review_at: Option<i64>,
    pub next_review_at: i64,
    pub state: CardState,
}

#[derive(Debug, Clone, Copy)]
pub struct CardInput {
    pub difficulty: f64,
    pub stability: f64,
    pub interval_days: f64,
    pub reps: i64,
    pub lapses: i64,
    pub last_review_at: Option<i64>,
    pub state: CardState,
}

impl Default for CardInput {
    fn default() -> Self {
        Self {
            difficulty: 5.0,
            stability: 2.0,
            interval_days: 0.0,
            reps: 0,
            lapses: 0,
            last_review_at: None,
            state: CardState::New,
        }
    }
}

const DECAY: f64 = -0.5;
const R_TARGET: f64 = 0.9;

/// Retrievability: probability of recall after `elapsed_days` with stability S.
/// R(t, S) = (1 + t / (9 * S))^(-1)
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64 {
    if stability <= 0.0 {
        return 0.0;
    }
    (1.0 + elapsed_days / (9.0 * stability)).powf(-1.0)
}

/// Compute the next interval (days) to wait for the next review.
/// Uses desired retention of 0.9.
pub fn next_interval(stability: f64) -> f64 {
    if stability <= 0.0 {
        return 0.0;
    }
    // I = S * 9 * (1/R - 1)
    // For R=0.9: I = S
    // For R=0.85: I ≈ 1.588 * S
    let i = 9.0 * stability * (1.0 / R_TARGET - 1.0);
    i.max(0.0)
}

/// Initialize a new card's schedule based on first rating.
fn init_schedule(rating: Rating, now: i64) -> CardSchedule {
    let r = rating as i64 as f64;
    let w = FSRSWeights::default().w;
    // S0 = w[r-1]
    let s0 = w[(r - 1.0) as usize];
    // D0 = w4 - exp(w5) * (rating - 3)  (FSRS v4 formula)
    let d0 = (w[4] - (w[5] * (r - 3.0)).exp()).clamp(1.0, 10.0);

    let interval = if rating == Rating::Again {
        0.0
    } else if rating == Rating::Hard || rating == Rating::Good {
        1.0 // 1 day
    } else {
        4.0 // Easy: 4 days
    };

    CardSchedule {
        difficulty: d0,
        stability: s0,
        interval_days: interval,
        reps: 1,
        lapses: 0,
        last_review_at: Some(now),
        next_review_at: now + (interval * 86400.0) as i64,
        state: if rating == Rating::Again { CardState::Learning } else { CardState::Review },
    }
}

/// Review a card and compute its new schedule.
pub fn review(input: CardInput, rating: Rating, now: i64) -> CardSchedule {
    let w = FSRSWeights::default().w;
    let _r = rating as i64 as f64;

    let elapsed_days = if let Some(last) = input.last_review_at {
        (now - last) as f64 / 86400.0
    } else {
        0.0
    };

    let r_score = retrievability(elapsed_days.max(0.0), input.stability);

    // Forgot (Again): use DIFFICULTY in formula (not stability!)
    if rating == Rating::Again {
        // S'_Again = w[15] * D^(-w[16]) * exp(w[14] * (1 - R))
        let new_s = (w[15] * input.difficulty.powf(-w[16]) * (w[14] * (1.0 - r_score)).exp())
            .max(0.01);
        let new_d = (input.difficulty - w[6] * 2.0).clamp(1.0, 10.0);
        return CardSchedule {
            difficulty: new_d,
            stability: new_s,
            interval_days: 0.0,
            reps: input.reps + 1,
            lapses: input.lapses + 1,
            last_review_at: Some(now),
            next_review_at: now + 600,
            state: CardState::Relearning,
        };
    }

    let s_inc = match rating {
        Rating::Hard => w[13] * r_score,
        Rating::Good => 1.0 + (w[8] + w[9] * r_score) * 0.1,
        Rating::Easy => w[10] * input.stability,
        _ => 1.0,
    };
    let new_s = (input.stability * s_inc).max(0.01);

    let d_delta = match rating {
        Rating::Hard => -w[12],
        Rating::Good => 0.0,
        Rating::Easy => -w[12] * 0.5,
        _ => 0.0,
    };
    let new_d = (input.difficulty + d_delta).clamp(1.0, 10.0);

    let new_interval = next_interval(new_s).ceil().max(1.0);
    CardSchedule {
        difficulty: new_d,
        stability: new_s,
        interval_days: new_interval,
        reps: input.reps + 1,
        lapses: input.lapses,
        last_review_at: Some(now),
        next_review_at: now + (new_interval * 86400.0) as i64,
        state: CardState::Review,
    }
}

/// Create a brand-new card (never reviewed) — schedule a first review.
#[allow(dead_code)]
pub fn new_card(now: i64) -> CardSchedule {
    CardSchedule {
        difficulty: 5.0,
        stability: 2.0,
        interval_days: 0.0,
        reps: 0,
        lapses: 0,
        last_review_at: None,
        next_review_at: now,
        state: CardState::New,
    }
}

/// Review a new card for the first time.
pub fn first_review(_input: CardInput, rating: Rating, now: i64) -> CardSchedule {
    init_schedule(rating, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrievability() {
        // At t=0, R=1
        assert!((retrievability(0.0, 10.0) - 1.0).abs() < 0.001);
        // At t=9*S, R=0.5
        assert!((retrievability(90.0, 10.0) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_first_review_easy() {
        let s = first_review(CardInput::default(), Rating::Easy, 1_000_000_000);
        assert_eq!(s.reps, 1);
        assert!(s.interval_days > 1.0);
        assert_eq!(s.state, CardState::Review);
    }

    #[test]
    fn test_repeated_again_decreases_stability() {
        let mut s = first_review(CardInput::default(), Rating::Good, 1_000_000_000);
        let stable_after_good = s.stability;

        // Simulate forgetting after 30 days
        let later = 1_000_000_000 + 30 * 86400;
        s = review(
            CardInput {
                difficulty: s.difficulty,
                stability: s.stability,
                interval_days: s.interval_days,
                reps: s.reps,
                lapses: s.lapses,
                last_review_at: s.last_review_at,
                state: s.state,
            },
            Rating::Again,
            later,
        );
        // After forgetting, stability should be lower
        assert!(s.stability < stable_after_good);
        assert_eq!(s.lapses, 1);
    }

    #[test]
    fn test_subsequent_good_increases_interval() {
        let s1 = first_review(CardInput::default(), Rating::Good, 1_000_000_000);
        let s2 = review(
            CardInput {
                difficulty: s1.difficulty,
                stability: s1.stability,
                interval_days: s1.interval_days,
                reps: s1.reps,
                lapses: s1.lapses,
                last_review_at: s1.last_review_at,
                state: s1.state,
            },
            Rating::Good,
            1_000_000_000 + (s1.interval_days * 86400.0) as i64,
        );
        // Interval should grow
        assert!(s2.interval_days > s1.interval_days);
    }
}
