use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use std::sync::{Arc, Mutex};

// ============================================
// SITE & AUTH
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteProfile {
    pub name: String,
    pub url: String,
    pub phone_selector: String,
    pub password_selector: String,
    pub login_button_selector: String,
    pub bet_amount_selector: String,
    pub bet_submit_selector: String,
    pub logged_in_indicator: Option<String>,
    pub login_form_indicator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LoginStatus {
    Unknown,
    NotLoggedIn,
    LoggedIn,
    LoginFailed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    SiteSelection,
    LoadingSite,
    LoginDialog,
    LoginInProgress,
    SiteLoaded,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub login_status: LoginStatus,
    pub phone: String,
    pub bet_amount: String,
    pub last_url: String,
    pub timestamp: u64,
    pub network_was_up: bool,
    pub is_dark_mode: bool,
}

// ============================================
// BALANCE
// ============================================

#[derive(Debug, Clone, Deserialize)]
pub struct AviatorBalance {
    pub success: bool,
    #[serde(default)]
    pub balance: Option<String>,
    #[serde(default)]
    pub balance_text: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub raw_text: Option<String>,
    #[serde(default)]
    pub pending: Option<bool>,
    #[serde(default)]
    pub has_amount_span: Option<bool>,
    #[serde(default)]
    pub has_currency_span: Option<bool>,
    #[serde(default)]
    pub has_header_right: Option<bool>,
    #[serde(default)]
    pub has_header: Option<bool>,
    #[serde(default)]
    pub angular_ready: Option<bool>,
    #[serde(default)]
    pub body_ready: Option<bool>,
}

// ============================================
// GAME STATE (from browser)
// ============================================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GameState {
    pub bet_button_class: String,
    pub bet_button_text: String,
    pub cashout_amount: Option<String>,
    pub input_value: Option<String>,
    pub is_waiting: bool,
    pub is_cancel_without_tooltip: bool,
    pub is_cashout_available: bool,
    pub tooltip_text: Option<String>,
    pub raw_html: String,
}

// ============================================
// GAME PHASE — STATE MACHINE
// ============================================

#[derive(Debug, Clone, PartialEq)]
pub enum GamePhase {
    Idle,           // Can bet, fetch balance, fetch payouts
    Betting,        // Bet sent, waiting for browser confirmation
    WaitingRound,   // Bet locked, waiting for plane to take off
    Flying,         // Plane flying, monitoring for cashout target
    CashoutPending, // Cashout clicked, waiting for confirmation
    Settling,       // Round ended, updating history/balance
    Error(String),  // Unrecoverable error state
}

impl GamePhase {
    pub fn can_fetch_balance(&self) -> bool {
        matches!(self, GamePhase::Idle | GamePhase::WaitingRound | GamePhase::Settling)
    }

    pub fn can_fetch_payouts(&self) -> bool {
        // Payouts are passive, always okay
        true
    }

    pub fn can_place_bet(&self) -> bool {
        *self == GamePhase::Idle
    }

    pub fn can_cashout(&self) -> bool {
        *self == GamePhase::Flying
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, GamePhase::Betting | GamePhase::CashoutPending)
    }
}

// ============================================
// BROWSER EVENTS — EVENT-DRIVEN ARCHITECTURE
// ============================================

#[derive(Debug, Clone)]
pub enum BrowserEvent {
    PayoutChanged(Vec<String>),
    BalanceChanged(String),
    GameStateChanged(GameState),
    BetConfirmed,
    CashoutConfirmed(f64),
    RoundCrashed,
    Error(String),
    NetworkStatus(bool),
}

// ============================================
// GAME HISTORY & STRATEGY
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRound {
    pub round_id: String,
    pub timestamp: u64,
    pub odd_used: f64,
    pub multiplier_used: f64,
    pub bet_amount: f64,
    pub cashout_target: f64,
    pub actual_cashout: Option<f64>,
    pub result: RoundResult,
    pub balance_before: String,
    pub balance_after: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoundResult {
    Win,
    Loss,
    Pending,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameHistory {
    pub rounds: Vec<GameRound>,
    pub total_wins: u32,
    pub total_losses: u32,
    pub current_strategy_index: usize,
    pub last_updated: u64,
}

impl GameHistory {
    pub fn new() -> Self {
        Self {
            rounds: Vec::new(),
            total_wins: 0,
            total_losses: 0,
            current_strategy_index: 0,
            last_updated: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn add_round(&mut self, round: GameRound) {
        match round.result {
            RoundResult::Win => self.total_wins += 1,
            RoundResult::Loss => self.total_losses += 1,
            _ => {}
        }
        self.rounds.push(round);
        self.last_updated = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn get_last_round(&self) -> Option<&GameRound> {
        self.rounds.last()
    }

    pub fn advance_strategy(&mut self, strategy_len: usize) {
        if strategy_len > 0 {
            self.current_strategy_index = (self.current_strategy_index + 1) % strategy_len;
        }
    }

    pub fn reset_strategy(&mut self) {
        self.current_strategy_index = 0;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgoStrategy {
    pub tuples: Vec<(f64, f64)>,
    pub expected_amount: String,
    pub created_at: u64,
}

impl AlgoStrategy {
    pub fn get_current_tuple(&self, index: usize) -> Option<(f64, f64)> {
        self.tuples.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.tuples.len()
    }
}

// ============================================
// NETWORK MONITOR
// ============================================

#[derive(Clone)]
pub struct NetworkMonitor {
    pub is_online: Arc<Mutex<bool>>,
    pub last_check: Arc<Mutex<SystemTime>>,
    pub is_checking: bool,
}

impl NetworkMonitor {
    pub fn new() -> Self {
        Self {
            is_online: Arc::new(Mutex::new(true)),
            last_check: Arc::new(Mutex::new(SystemTime::now())),
            is_checking: false,
        }
    }

    pub fn check_online(&self) -> bool {
        let last = *self.last_check.lock().unwrap();
        if last.elapsed().unwrap_or(Duration::MAX) < Duration::from_secs(30) {
            *self.is_online.lock().unwrap()
        } else {
            true
        }
    }

    pub fn set_online(&self, online: bool) {
        *self.is_online.lock().unwrap() = online;
        *self.last_check.lock().unwrap() = SystemTime::now();
    }
}

// ============================================
// PERSISTENCE
// ============================================

pub fn save_state(snapshot: &AppStateSnapshot) {
    if let Ok(json) = serde_json::to_string(snapshot) {
        let _ = std::fs::write("aviet_state.json", json);
    }
}

pub fn load_state() -> Option<AppStateSnapshot> {
    if let Ok(content) = std::fs::read_to_string("aviet_state.json") {
        if let Ok(state) = serde_json::from_str(&content) {
            return Some(state);
        }
    }
    None
}

pub fn clear_state() {
    let _ = std::fs::remove_file("aviet_state.json");
}

pub fn save_game_history(history: &GameHistory) {
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = std::fs::write("aviet_game_history.json", json);
    }
}

pub fn load_game_history() -> Option<GameHistory> {
    if let Ok(content) = std::fs::read_to_string("aviet_game_history.json") {
        if let Ok(history) = serde_json::from_str(&content) {
            return Some(history);
        }
    }
    None
}

pub fn save_algo_strategy(strategy: &AlgoStrategy) {
    if let Ok(json) = serde_json::to_string_pretty(strategy) {
        let _ = std::fs::write("aviet_algo_strategy.json", json);
    }
}

pub fn load_algo_strategy() -> Option<AlgoStrategy> {
    if let Ok(content) = std::fs::read_to_string("aviet_algo_strategy.json") {
        if let Ok(strategy) = serde_json::from_str(&content) {
            return Some(strategy);
        }
    }
    None
}

pub fn load_site() -> SiteProfile {
    let path = std::path::Path::new("sites.json");
    let mut default = SiteProfile {
        name: "Betika".to_string(),
        url: "https://www.betika.com/en-ke/".to_string(),
        phone_selector: "input[name='phone-number']".to_string(),
        password_selector: "input[type='password']".to_string(),
        login_button_selector: "button[type='submit']".to_string(),
        bet_amount_selector: ".bet-amount-input".to_string(),
        bet_submit_selector: ".place-bet-button".to_string(),
        logged_in_indicator: Some("span.nav__item__label, a[href*='profile']".to_string()),
        login_form_indicator: Some("input[name='phone-number']".to_string()),
    };

    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(sites) = json.get("sites") {
                    if let Ok(site_vec) = serde_json::from_value::<Vec<SiteProfile>>(sites.clone()) {
                        if let Some(site) = site_vec.into_iter().find(|s| s.name == "Betika") {
                            default = site;
                        }
                    }
                }
            }
        }
    }

    default
}

pub fn escape_js_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}