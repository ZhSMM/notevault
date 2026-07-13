// Card commands: list due, get, review, stats, reindex

use crate::commands::notes::Card;
use crate::error::{VaultError, VaultResult};
use crate::fsrs::{self, CardInput, CardState, Rating};
use crate::state::AppState;
use std::path::PathBuf;

#[tauri::command]
pub fn list_due_cards(
    state: tauri::State<AppState>,
    limit: Option<u32>,
) -> VaultResult<Vec<Card>> {
    let now = chrono::Utc::now().timestamp();
    state.with_db(|db| db.list_due_cards(now, limit.unwrap_or(50)))
}

#[tauri::command]
pub fn get_card(
    state: tauri::State<AppState>,
    id: String,
) -> VaultResult<Option<Card>> {
    state.with_db(|db| db.get_card(&id))
}

#[tauri::command]
pub fn count_due_cards(state: tauri::State<AppState>) -> VaultResult<i64> {
    let now = chrono::Utc::now().timestamp();
    state.with_db(|db| db.count_due_cards(now))
}

#[tauri::command]
pub fn count_total_cards(state: tauri::State<AppState>) -> VaultResult<i64> {
    state.with_db(|db| db.count_cards())
}

#[derive(Debug, serde::Serialize)]
pub struct ReviewResult {
    pub card: Card,
    pub next_due_in_days: f64,
    pub correct: bool,
}

#[tauri::command]
pub fn review_card(
    state: tauri::State<AppState>,
    id: String,
    rating: u8,
) -> VaultResult<ReviewResult> {
    let now = chrono::Utc::now().timestamp();
    let rating = Rating::from_u8(rating).ok_or_else(|| {
        VaultError::Other(format!("Invalid rating: {} (must be 1-4)", rating))
    })?;
    state.with_db(|db| {
        let mut card = db.get_card(&id)?
            .ok_or_else(|| VaultError::Other(format!("Card not found: {}", id)))?;
        let input = CardInput {
            difficulty: card.difficulty,
            stability: card.stability,
            interval_days: card.interval_days,
            reps: card.reps,
            lapses: card.lapses,
            last_review_at: card.last_review_at,
            state: match card.state.as_str() {
                "new" => CardState::New,
                "learning" => CardState::Learning,
                "review" => CardState::Review,
                "relearning" => CardState::Relearning,
                _ => CardState::New,
            },
        };
        let new_schedule = if card.reps == 0 {
            fsrs::first_review(input, rating, now)
        } else {
            fsrs::review(input, rating, now)
        };
        card.difficulty = new_schedule.difficulty;
        card.stability = new_schedule.stability;
        card.interval_days = new_schedule.interval_days;
        card.reps = new_schedule.reps;
        card.lapses = new_schedule.lapses;
        card.last_review_at = new_schedule.last_review_at;
        card.next_review_at = new_schedule.next_review_at;
        card.state = match new_schedule.state {
            CardState::New => "new".into(),
            CardState::Learning => "learning".into(),
            CardState::Review => "review".into(),
            CardState::Relearning => "relearning".into(),
        };
        db.save_card(&card)?;
        let next_due_in_days = (card.next_review_at - now) as f64 / 86400.0;
        let correct = rating != Rating::Again;
        Ok(ReviewResult { card, next_due_in_days, correct })
    })
}

#[derive(Debug, serde::Serialize)]
pub struct CardStats {
    pub total: i64,
    pub due: i64,
    pub new_count: i64,
    pub learning: i64,
    pub review: i64,
    pub relearning: i64,
    pub total_reviews: i64,
    pub total_lapses: i64,
    pub avg_difficulty: f64,
    pub avg_stability: f64,
}

#[tauri::command]
pub fn card_stats(state: tauri::State<AppState>) -> VaultResult<CardStats> {
    let now = chrono::Utc::now().timestamp();
    state.with_db(|db| {
        let total = db.count_cards()?;
        let due = db.count_due_cards(now)?;
        let new_count: i64 = db.conn.query_row("SELECT COUNT(*) FROM cards WHERE state='new'", [], |r| r.get(0))?;
        let learning: i64 = db.conn.query_row("SELECT COUNT(*) FROM cards WHERE state='learning'", [], |r| r.get(0))?;
        let review: i64 = db.conn.query_row("SELECT COUNT(*) FROM cards WHERE state='review'", [], |r| r.get(0))?;
        let relearning: i64 = db.conn.query_row("SELECT COUNT(*) FROM cards WHERE state='relearning'", [], |r| r.get(0))?;
        let total_reviews: i64 = db.conn.query_row("SELECT COALESCE(SUM(reps), 0) FROM cards", [], |r| r.get(0))?;
        let total_lapses: i64 = db.conn.query_row("SELECT COALESCE(SUM(lapses), 0) FROM cards", [], |r| r.get(0))?;
        let avg_d: f64 = db.conn.query_row("SELECT COALESCE(AVG(difficulty), 0) FROM cards", [], |r| r.get(0))?;
        let avg_s: f64 = db.conn.query_row("SELECT COALESCE(AVG(stability), 0) FROM cards", [], |r| r.get(0))?;
        Ok(CardStats {
            total, due, new_count, learning, review, relearning,
            total_reviews, total_lapses, avg_difficulty: avg_d, avg_stability: avg_s,
        })
    })
}

#[tauri::command]
pub fn reindex_cards(
    state: tauri::State<AppState>,
    note_path: Option<String>,
) -> VaultResult<ReindexCardsResult> {
    use crate::commands::notes::{extract_cards, list_notes_inner};
    let info = state.info().ok_or(VaultError::NoVaultOpen)?;
    let base = PathBuf::from(&info.path);
    let notes = if let Some(p) = note_path {
        vec![crate::commands::notes::NoteLite { path: p }]
    } else {
        let n = list_notes_inner(&base, None)?;
        n.into_iter().map(|m| crate::commands::notes::NoteLite { path: m.path }).collect()
    };
    let mut notes_indexed = 0;
    let mut cards_total = 0;
    for n in &notes {
        let full = base.join(&n.path);
        let raw = std::fs::read_to_string(&full).unwrap_or_default();
        let (_fm, body) = crate::commands::notes::parse_frontmatter(&raw);
        let cards = extract_cards(&body, &n.path);
        let count = cards.len();
        state.with_db(|db| db.replace_cards(&cards))?;
        notes_indexed += 1;
        cards_total += count;
    }
    Ok(ReindexCardsResult { notes_indexed, cards_total })
}

#[derive(Debug, serde::Serialize)]
pub struct ReindexCardsResult {
    pub notes_indexed: usize,
    pub cards_total: usize,
}
