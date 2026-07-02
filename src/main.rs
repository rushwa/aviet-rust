mod models;
mod selectors;
mod dom_helpers;
mod browser;
mod algo;

use gtk::prelude::*;
use gtk::{glib, Align, Orientation, PolicyType};
use relm4::prelude::*;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use gtk4::NaturalWrapMode;
use gtk4::pango::EllipsizeMode;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use models::*;
use browser::{BrowserController, BrowserCommand, BrowserEvent, GameState};
use dom_helpers::DOM_HELPERS_SCRIPT;
use crate::algo::algo::{float_eq, multiply_rounded};

#[derive(Clone)]
struct AsyncBridge {
    runtime: Arc<Runtime>,
    browser_tx: mpsc::UnboundedSender<BrowserCommand>,
}

impl AsyncBridge {
    fn new(browser_tx: mpsc::UnboundedSender<BrowserCommand>) -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime")
        );
        Self { runtime, browser_tx }
    }

    fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(future);
    }

    fn send_browser_cmd(&self, cmd: BrowserCommand) {
        let _ = self.browser_tx.send(cmd);
    }
}

// ============================================
// MESSAGES
// ============================================

#[derive(Debug, Clone)]
enum AppMsg {
    StartBrowser,
    BrowserStarted(Result<String, String>),
    StopBrowser,
    NavigateTo(String),
    NavigateComplete(Result<String, String>),
    PerformLogin,
    ExecuteLogin(String, String),
    LoginCompleted(Result<String, String>),
    ValidateSession,
    SessionValid(bool),
    GetSessionInfo,
    SessionInfoReceived(Result<serde_json::Value, String>),
    SaveSessionToStorage(serde_json::Value),
    LoadSessionFromStorage,
    SessionLoadedFromStorage(Result<serde_json::Value, String>),
    RestoreSession,
    InjectDomApi,
    DomApiReady(bool),
    CheckDomApiReady,
    NavigateToAviator,
    AviatorNavigated(Result<String, String>),
    CheckAviatorReady,
    AviatorReady,
    AviatorPageDetected,
    FetchPayouts,
    PayoutsReceived(Result<Vec<String>, String>),
    StartPayoutWatcher,
    StopPayoutWatcher,
    TogglePayoutWatcher,
    NewPayoutReceived(String),
    GetAviatorBalance,
    AviatorBalanceReceived(Result<AviatorBalance, String>),
    PollAviatorBalance,
    PlaceBet(String, String, String),
    Cashout,
    BetResult(Result<String, String>),
    ClickAviatorDemo,
    AviatorDemoResult(Result<String, String>),
    InitializeGame,
    MonitorGame,
    PauseGame,
    StopGame,
    ResumePlay(String, String),
    FetchAlgoStrategy,
    AlgoStrategyReceived(Result<Vec<(f64, f64)>, String>),
    GetGameStrategy(String, String, String, String, String),
    SelectSite(SiteProfile),
    PhoneChanged(String),
    PasswordChanged(String),
    SaveCredentials(bool),
    BetAmountChanged(String),
    ToggleTheme,
    RefreshPage,
    CloseSite,
    CheckNetwork,
    NetworkStatusChanged(bool),
    RetryConnection,
    PageLoadError(String),
    UpdateStatus(String),
    ScriptExecuted(String),
    CheckBalance,
    BalanceChecked(Result<String, String>),
    Deposit(String),
    Withdraw(String),
    LoadPlayHistory,
    SavePlayHistory,
    ClearPlayHistory,
    GetRoundHistory,
    SaveRoundHistory,
    LogMessage(String),
    ClearLogs,
    // GAME AUTOMATION MESSAGES
    StartAutoPlay,
    StopAutoPlay,
    AutoPlayTick,
    GameStateReceived(Result<GameState, String>),
    AutoBetPlaced(Result<String, String>),
    AutoCashoutResult(Result<String, String>),
    StrategyTupleUsed(usize),
    RoundCompleted { result: RoundResult, amount: f64 },
    CheckWinReflected,
    WinReflected(bool),
    AlgoStartChanged(String),
    AlgoEndChanged(String),
    AlgoProfitierChanged(String),
    AlgoMultiplierChanged(String),
    AlgoItemNoChanged(String),
    AlgoChoiceChanged(usize),
    ClickDemoButton,
    // EVENT-DRIVEN MESSAGES
    BrowserEventReceived(BrowserEvent),
}

// ============================================
// APP COMPONENT
// ============================================

struct App {
    state: AppState,
    site: SiteProfile,
    login_status: LoginStatus,
    phone: String,
    password: String,
    save_credentials: bool,
    bet_amount: String,
    status_message: String,
    async_bridge: Option<AsyncBridge>,
    payout_watcher_active: bool,
    current_balance: String,
    payouts: Vec<String>,
    algo_strategy: String,
    session_data: Option<serde_json::Value>,
    dom_api_ready: bool,
    is_dark_mode: bool,
    session_restored: bool,
    last_loaded_url: String,
    network_monitor: NetworkMonitor,
    logs: Vec<String>,
    log_counter: usize,
    // GAME AUTOMATION STATE
    auto_play_active: bool,
    game_history: GameHistory,
    algo_strategy_data: Option<AlgoStrategy>,
    current_odd: f64,
    current_multiplier: f64,
    cashout_target: f64,
    is_first_play: bool,
    pending_round: Option<GameRound>,
    algo_start: String,
    algo_end: String,
    algo_profitier: String,
    algo_multiplier: String,
    algo_item_no: String,
    algo_choice: usize,
    is_demo_mode: bool,
    previous_was_cashout: bool,
    last_bet_timestamp: u64,
    auto_play_timer_active: bool,
    pending_cashout_click: bool,
    cashout_click_time: u64,
    // === NEW: Combined architecture ===
    game_phase: GamePhase,
    browser_busy: bool,
    command_queue: Vec<BrowserCommand>,
    next_tick_scheduled: bool,
}

#[relm4::component]
impl SimpleComponent for App {
    type Input = AppMsg;
    type Output = ();
    type Init = ();

    view! {
        #[root]
        gtk::ApplicationWindow {
            set_size_request:(1400, 900),
            set_resizable:false,
            set_title: Some("Aviet - Gianna"),
            set_default_width: 1400,
            set_default_height: 900,
            gtk::Box {
                set_orientation: Orientation::Horizontal,
                set_size_request: (1400,900),

                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    set_margin_all: 12,
                    set_size_request: (400,720),
                    set_expand: false,
                    gtk::Box{
                        set_orientation: Orientation::Horizontal,
                        set_hexpand: true,
                        set_valign:Align::Start,
                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_margin_all: 12,
                        set_spacing: 10,
                        set_width_request: 400,

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_spacing: 12,
                            set_margin_all:12,
                            set_hexpand: true,
                            set_vexpand: false,

                            gtk::Box{
                                set_margin_all: 8,
                                add_css_class: "logo",
                                set_halign: Align::Start,

                                gtk::Label {
                                    set_label: "Aviet Gianna",
                                    add_css_class: "logo-title",
                                    set_hexpand: true,
                                }
                            },

                            gtk::Button {
                                set_margin_all: 18,
                                #[watch]
                                set_icon_name: if model.is_dark_mode {
                                    "weather-clear-symbolic"
                                } else {
                                    "weather-clear-night-symbolic"
                                },
                                set_tooltip_text: Some("Toggle theme"),
                                connect_clicked => AppMsg::ToggleTheme,
                            },
                        },

                        gtk::Label {
                            set_label: "thirtyfour + Relm4 Automation",
                            set_xalign: 0.5,
                            add_css_class: "dim-label",
                        },

                        gtk::Separator {
                            set_width_request: 400,
                        },

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_spacing: 20,
                            set_margin_all: 12,
                            gtk::Button {
                                set_label: "Start Browser",
                                add_css_class: "suggested-action",
                                set_margin_start:50,
                                set_margin_end:50,
                                set_halign: Align::Start,
                                #[watch]
                                set_sensitive: model.async_bridge.is_none(),
                                connect_clicked => AppMsg::StartBrowser,
                            },

                            gtk::Button {
                                set_label: "Stop",
                                add_css_class: "destructive-action",
                                set_margin_start: 50,
                                set_margin_end: 1,
                                set_halign: Align::End,
                                #[watch]
                                set_sensitive: model.async_bridge.is_some(),
                                connect_clicked => AppMsg::StopBrowser,
                            }
                        },
                        gtk::Separator {
                            set_width_request: 400,
                        },
                    },
                },

                gtk::Box{
                    set_orientation: Orientation::Horizontal,
                    set_vexpand: true,
                    set_margin_all: 12,
                    gtk::ScrolledWindow {
                        set_hscrollbar_policy: PolicyType::Never,
                        set_vscrollbar_policy: PolicyType::Automatic,
                        set_hexpand: true,
                        set_vexpand: true,
                        set_margin_all: 12,

                        gtk::Box {
                            set_orientation: Orientation::Vertical,
                            set_margin_all: 20,
                            set_spacing: 20,
                            set_hexpand: true,
                            set_vexpand: true,

                            gtk::Box {
                                set_orientation: Orientation::Vertical,
                                set_spacing: 12,

                                gtk::Box {
                                    set_orientation: Orientation::Horizontal,
                                    set_spacing: 8,

                                    gtk::Image {
                                        #[watch]
                                        set_icon_name: if model.network_monitor.check_online() {
                                            Some("network-wired-symbolic")
                                        } else {
                                            Some("network-offline-symbolic")
                                        },
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: if model.network_monitor.check_online() {
                                            "Online"
                                        } else {
                                            "Offline"
                                        },
                                    },
                                },

                                gtk::Separator {},

                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 4,

                                    gtk::Label {
                                        set_label: "SITE",
                                        set_xalign: 0.0,
                                        add_css_class: "heading",
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.site.name,
                                        add_css_class: "title-3",
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: &model.site.url,
                                        add_css_class: "dim-label",
                                        set_ellipsize: EllipsizeMode::End,
                                    },
                                },

                                gtk::Separator {},

                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 8,

                                    gtk::Label {
                                        set_label: "Authentication",
                                        set_xalign: 0.0,
                                        add_css_class: "heading",
                                    },

                                    gtk::Entry {
                                        set_placeholder_text: Some("Phone Number (0712...)"),
                                        set_text: &model.phone,
                                        connect_changed[sender] => move |entry| {
                                            sender.input(AppMsg::PhoneChanged(entry.text().to_string()));
                                        }
                                    },

                                    gtk::Entry {
                                        set_placeholder_text: Some("Password"),
                                        set_visibility: false,
                                        set_text: &model.password,
                                        connect_changed[sender] => move |entry| {
                                            sender.input(AppMsg::PasswordChanged(entry.text().to_string()));
                                        }
                                    },

                                    gtk::CheckButton {
                                        set_label: Some("Save credentials"),
                                        connect_toggled[sender] => move |btn| {
                                            sender.input(AppMsg::SaveCredentials(btn.is_active()));
                                        }
                                    },

                                    gtk::Button {
                                        set_label: "Auth Initiate",
                                        add_css_class: "suggested-action",
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some() && model.state != AppState::LoginInProgress,
                                        connect_clicked => AppMsg::PerformLogin,
                                    },

                                    gtk::ProgressBar {
                                        #[watch]
                                        set_visible: model.state == AppState::LoginInProgress,
                                        #[watch]
                                        set_fraction: if model.state == AppState::LoginInProgress { 0.5 } else { 0.0 },
                                    },
                                },

                                gtk::Separator {},

                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 8,

                                    gtk::Label {
                                        set_label: "AVIATOR CONTROLS",
                                        set_xalign: 0.0,
                                        add_css_class: "heading",
                                    },

                                    gtk::Button {
                                        set_label: "Navigate to Aviator",
                                        add_css_class: "suggested-action",
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some() && model.login_status == LoginStatus::LoggedIn,
                                        connect_clicked => AppMsg::NavigateToAviator,
                                    },

                                    gtk::Button {
                                        set_label: "Fetch Payouts",
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some() && model.game_phase.can_fetch_payouts(),
                                        connect_clicked => AppMsg::FetchPayouts,
                                    },

                                    gtk::Button {
                                        set_label: if model.payout_watcher_active { "Stop Watcher" } else { "Start Watcher" },
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some(),
                                        connect_clicked[sender] => move |_| {
                                            sender.input(if model.payout_watcher_active {
                                                AppMsg::StopPayoutWatcher
                                            } else {
                                                AppMsg::StartPayoutWatcher
                                            });
                                        },
                                    },

                                    gtk::Button {
                                        set_label: "Get Balance",
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some() && model.game_phase.can_fetch_balance(),
                                        connect_clicked => AppMsg::GetAviatorBalance,
                                    },

                                    gtk::Button {
                                        set_label: "Play Demo",
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some(),
                                        connect_clicked => AppMsg::ClickAviatorDemo,
                                    },
                                },

                                gtk::Separator {},

                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 8,

                                    gtk::Label {
                                        set_label: "AUTO PLAY",
                                        set_xalign: 0.0,
                                        add_css_class: "heading",
                                    },

                                    gtk::Button {
                                        set_label: if model.auto_play_active { "Stop Auto Play" } else { "Start Auto Play" },
                                        add_css_class: if model.auto_play_active { "destructive-action" } else { "suggested-action" },
                                        #[watch]
                                        set_sensitive: model.async_bridge.is_some() && (model.login_status == LoginStatus::LoggedIn || model.is_demo_mode),
                                        connect_clicked => if model.auto_play_active {
                                            AppMsg::StopAutoPlay
                                        } else {
                                            AppMsg::StartAutoPlay
                                        },
                                    },

                                    gtk::Label {
                                        #[watch]
                                        set_label: &format!("Phase: {:?} | Cashout Target: {:.2?} | Wins: {} | Losses: {}",
                                            model.game_phase,
                                            model.cashout_target,
                                            model.game_history.total_wins,
                                            model.game_history.total_losses
                                        ),
                                        set_xalign: 0.0,
                                        add_css_class: "dim-label",
                                    },
                                },
                            },

                            gtk::Separator {},

                            gtk::Box {
                                set_orientation: Orientation::Horizontal,
                                set_spacing: 8,

                                gtk::Button {
                                    set_label: "Refresh",
                                    connect_clicked => AppMsg::RefreshPage,
                                },

                                gtk::Button {
                                    set_label: "Close",
                                    connect_clicked => AppMsg::CloseSite,
                                },
                            },

                            gtk::Separator {},
                        },
                    },
                },

                gtk::Box{
                    set_orientation: Orientation::Horizontal,
                    set_hexpand: false,
                    set_vexpand: false,
                    set_margin_all: 12,
                    set_valign: Align::End,

                    gtk::Box{
                        set_orientation:Orientation::Vertical,
                        set_margin_all: 12,
                            set_size_request: (400,80),

                        gtk::Frame {
                            set_label: Some("Status"),
                            set_hexpand: false,
                            set_vexpand: false,
                            set_margin_all: 10,
                                set_size_request: (380,70),

                            gtk::ScrolledWindow {
                                set_hscrollbar_policy: PolicyType::Never,
                                set_vscrollbar_policy: PolicyType::Always,
                                set_kinetic_scrolling: true,
                                set_margin_all: 10,
                                set_size_request: (380,70),

                                gtk::Label {
                                    set_size_request: (380,70),
                                    #[watch]
                                    set_label: &format!("{}",&model.status_message),
                                    set_margin_all: 2,
                                    set_natural_wrap_mode: NaturalWrapMode::Word,
                                    set_ellipsize: EllipsizeMode::End,
                                        set_lines: 10,
                                    set_hexpand: false,
                                    set_vexpand: false,
                                }
                            },
                        },
                    },
                },
            },

            gtk::Box {
                set_orientation: Orientation::Vertical,
                set_hexpand: false,
                set_vexpand: true,
                set_margin_all: 10,
                set_width_request: 1100,

                gtk::Label {
                    set_label: "Browser running externally (thirtyfour)",
                    add_css_class: "title-2",
                    set_margin_bottom: 12,
                },

                gtk::Box{
                    set_orientation: Orientation::Horizontal,
                    set_hexpand: true,
                    set_margin_all: 10,
                    set_spacing: 8,

                    gtk::Frame {
                        set_hexpand: true,
                        set_vexpand: false,
                        set_label: Some("Session Info / Logs"),
                            set_height_request: 240,
                            set_margin_all:2,

                        gtk::Box{
                            set_orientation: Orientation::Vertical,

                            set_margin_all:2,
                            gtk::Separator {},

                            gtk::ScrolledWindow {
                                set_min_content_height: 240,
                                set_max_content_height: 240,
                                set_hscrollbar_policy: PolicyType::Never,
                                set_vscrollbar_policy: PolicyType::Automatic,
                                set_margin_all:12,

                                gtk::Label {
                                    #[watch]
                                    set_label: &format!(
                                        "Browser Active: {}\nLogin Status: {:?}\nPhase: {:?}\nPayouts Tracked: {}\nWatcher Active: {}\nLast URL: {}\nDOM API: {}\nNetwork: {}\nTheme: {}\nSession Restored: {}\nBrowser Busy: {}\nQueue: {}",
                                        model.async_bridge.is_some(),
                                        model.login_status,
                                        model.game_phase,
                                        model.payouts.len(),
                                        model.payout_watcher_active,
                                        model.last_loaded_url,
                                        if model.dom_api_ready { "Ready" } else { "Not Ready" },
                                        if model.network_monitor.check_online() { "Online" } else { "Offline" },
                                        if model.is_dark_mode { "Dark" } else { "Light" },
                                        model.session_restored,
                                        model.browser_busy,
                                        model.command_queue.len()
                                    ),
                                    set_xalign: 0.0,
                                    set_yalign: 0.0,
                                    set_margin_all: 12,
                                    set_wrap: true,
                                }
                            }
                        },
                    },

                    gtk::Box{
                        set_orientation: Orientation::Vertical,
                        set_margin_all: 2,
                        set_spacing: 2,

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_spacing: 10,
                            set_margin_bottom: 10,
                            set_margin_end: 10,
                            set_halign: Align::End,

                            gtk::Label {
                                set_label: "Balance:",
                                set_xalign: 0.0,
                                add_css_class: "heading",
                            },

                            gtk::Label {
                                set_margin_all: 12,
                                #[watch]
                                set_label: &format!("KES {}", model.current_balance),
                                add_css_class: "title-3",
                            },
                        },

                        gtk::Separator {},

                        gtk::Frame {
                            set_label: Some("Live Payouts"),
                            set_vexpand: false,
                            set_hexpand: true,
                            set_valign: Align::End,
                            set_height_request: 250,
                            set_margin_top: 6,
                            set_margin_bottom: 0,

                            gtk::ScrolledWindow {
                                set_min_content_height: 150,
                                set_max_content_height: 200,
                                set_hscrollbar_policy: PolicyType::Never,
                                set_vscrollbar_policy: PolicyType::Automatic,

                                gtk::Label {
                                    #[watch]
                                    set_label: &model.payouts.join("\n"),
                                    set_natural_wrap_mode:NaturalWrapMode::Word,
                                    set_ellipsize:EllipsizeMode::End,
                                    set_lines: 10,
                                    // set_wrap: true,
                                    set_xalign: 0.0,
                                    set_yalign: 0.0,
                                    set_selectable:true,
                                    set_margin_all: 12,
                                }
                            }
                        },
                    },
                },

                gtk::Frame {
                    set_label: Some("Detailed Logs"),
                    set_margin_all: 10,
                    set_vexpand: true,

                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_margin_bottom:0 ,
                        set_spacing: 0,

                        gtk::Separator {},

                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hscrollbar_policy: PolicyType::Never,
                            set_vscrollbar_policy: PolicyType::Automatic,
                            set_kinetic_scrolling: true,
                            set_margin_all: 10,

                            gtk::Label {
                                #[watch]
                                set_label: &model.logs.join("\n"),
                                set_wrap: true,
                                set_xalign: 0.0,
                                set_yalign: 0.0,
                                set_margin_all: 8,
                                set_selectable: true,
                            }
                        },

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_all: 8,
                            set_halign: Align::End,

                            gtk::Button {
                                set_label: "Clear Logs",
                                connect_clicked => AppMsg::ClearLogs,
                            },
                        }
                    }
                },

                gtk::Frame {
                    set_label: Some("Algo Strategy"),
                    set_margin_all: 10,
                    set_vexpand: false,
                    set_valign: Align::End,

                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_margin_all: 7,
                        set_spacing: 5,

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_margin_all: 12,
                            set_spacing: 12,

                            gtk::Entry {
                                set_placeholder_text: Some("Start"),
                                set_text: &model.algo_start,
                                connect_changed[sender] => move |entry| {
                                    sender.input(AppMsg::AlgoStartChanged(entry.text().to_string()));
                                }
                            },

                            gtk::Entry {
                                set_placeholder_text: Some("End"),
                                set_text: &model.algo_end,
                                connect_changed[sender] => move |entry| {
                                    sender.input(AppMsg::AlgoEndChanged(entry.text().to_string()));
                                }
                            },

                            gtk::Entry {
                                set_placeholder_text: Some("Profitier"),
                                set_text: &model.algo_profitier,
                                connect_changed[sender] => move |entry| {
                                    sender.input(AppMsg::AlgoProfitierChanged(entry.text().to_string()));
                                }
                            },

                            gtk::Entry {
                                set_placeholder_text: Some("Multiplier"),
                                set_text: &model.algo_multiplier,
                                connect_changed[sender] => move |entry| {
                                    sender.input(AppMsg::AlgoMultiplierChanged(entry.text().to_string()));
                                }
                            },

                            gtk::SpinButton {
                                set_range: (1.0, 13.0),
                                set_increments: (1.0, 1.0),
                                set_value: 13.0,
                                connect_value_changed[sender] => move |btn| {
                                    sender.input(AppMsg::AlgoItemNoChanged(btn.value().to_string()));
                                }
                            },

                            gtk::DropDown {
                                set_model: Some(&gtk::StringList::new(&["Basic (0)", "Advanced (1)", "Multi Advanced (2)"])),
                                connect_selected_notify[sender] => move |dropdown| {
                                    sender.input(AppMsg::AlgoChoiceChanged(dropdown.selected() as usize));
                                }
                            },
                        },

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_margin_all: 12,
                            set_spacing: 15,

                            gtk::Button {
                                set_label: "Generate Strategy",
                                add_css_class: "suggested-action",
                                set_halign: Align::Start,
                                connect_clicked => AppMsg::FetchAlgoStrategy,
                            },

                            gtk::Label {
                                #[watch]
                                set_label: &model.algo_strategy,
                                // set_wrap: true,
                                set_hexpand: true,
                                set_halign: Align::Center,
                                set_natural_wrap_mode:NaturalWrapMode::Word,
                                set_ellipsize:EllipsizeMode::End,
                                set_lines:10,
                                set_xalign: 0.5,
                                set_margin_all:12,
                            },

                            gtk::Label {
                                #[watch]
                                set_label: &format!("Expected Balance: {}", model.algo_strategy_data.as_ref().map(|s| s.expected_amount.clone()).unwrap_or_else(|| "N/A".to_string())),
                                set_halign: Align::End,
                                set_xalign: 0.9,
                                add_css_class: "dim-label",
                            }
                        },
                    },
                }
            }
        }
    }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(include_str!("../style.css"));
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let site = load_site();
        let network_monitor = NetworkMonitor::new();

        let mut restored_phone = String::new();
        let mut restored_bet = "100".to_string();
        let mut restored_dark_mode = true;

        if let Some(snapshot) = load_state() {
            let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
            if now - snapshot.timestamp < 3600 {
                restored_phone = snapshot.phone;
                restored_bet = snapshot.bet_amount;
                restored_dark_mode = snapshot.is_dark_mode;
            } else {
                clear_state();
            }
        }

        if !restored_dark_mode {
            if let Some(settings) = gtk::Settings::default() {
                settings.set_gtk_application_prefer_dark_theme(false);
            }
        }

        let mut model = App {
            state: AppState::SiteSelection,
            site: site.clone(),
            login_status: LoginStatus::Unknown,
            phone: restored_phone,
            password: String::new(),
            save_credentials: false,
            bet_amount: restored_bet,
            status_message: "Click 'Start Browser' to launch".to_string(),
            async_bridge: None,
            payout_watcher_active: false,
            current_balance: "0.00".to_string(),
            payouts: vec![],
            algo_strategy: "Not loaded".to_string(),
            session_data: None,
            dom_api_ready: false,
            is_dark_mode: restored_dark_mode,
            session_restored: false,
            last_loaded_url: site.url.clone(),
            network_monitor,
            logs: vec!["[INIT] Application started".to_string()],
            log_counter: 1,
            auto_play_active: false,
            game_history: GameHistory::new(),
            algo_strategy_data: None,
            current_odd: 0.0,
            current_multiplier: 0.0,
            cashout_target: 0.0,
            is_first_play: false,
            pending_round: None,
            algo_start: "5".to_string(),
            algo_end: "1000".to_string(),
            algo_profitier: "5".to_string(),
            algo_multiplier: "2".to_string(),
            algo_item_no: "13".to_string(),
            algo_choice: 0,
            is_demo_mode:false,
            previous_was_cashout: false,
            last_bet_timestamp: 0,
            auto_play_timer_active: false,
            pending_cashout_click: false,
            cashout_click_time: 0,
            // === NEW ===
            game_phase: GamePhase::Idle,
            browser_busy: false,
            command_queue: Vec::new(),
            next_tick_scheduled: false,
        };
        model.add_log("[INIT] State restored (if available)");
        model.add_log(&format!("[INIT] Site: {} | URL: {}", site.name, site.url));

        let widgets = view_output!();

        // Network check timer — less frequent, state-gated
        let sender_net = sender.clone();
        glib::timeout_add_local(Duration::from_secs(10), move || {
            sender_net.input(AppMsg::CheckNetwork);
            glib::ControlFlow::Continue
        });

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            // ============================================
            // BROWSER LIFECYCLE
            // ============================================
            AppMsg::StartBrowser => {
                self.add_log("[BROWSER] Starting WebDriver browser...");
                self.status_message = "Starting WebDriver browser...".to_string();

                let (browser_tx, mut browser_rx) = mpsc::unbounded_channel::<BrowserCommand>();
                let (event_tx, mut event_rx) = mpsc::unbounded_channel::<BrowserEvent>();

                let bridge = AsyncBridge::new(browser_tx.clone());
                self.async_bridge = Some(bridge.clone());

                let sender_clone = sender.clone();
                sender_clone.input(AppMsg::LogMessage("[BROWSER] Spawning browser task...".to_string()));

                // Spawn browser command handler
                bridge.spawn(async move {
                    match BrowserController::new(event_tx.clone()).await {
                        Ok(controller) => {
                            let ctrl = Arc::new(tokio::sync::Mutex::new(controller));
                            sender_clone.input(AppMsg::BrowserStarted(Ok("Browser started".to_string())));
                            sender_clone.input(AppMsg::LogMessage("[BROWSER] WebDriver initialized successfully".to_string()));

                            while let Some(cmd) = browser_rx.recv().await {
                                let ctrl = ctrl.clone();
                                let sender = sender_clone.clone();

                                tokio::spawn(async move {
                                    let mut browser = ctrl.lock().await;
                                    match cmd {
                                        BrowserCommand::Navigate(url) => {
                                            let result = browser.navigate(&url).await;
                                            sender.input(AppMsg::NavigateComplete(result));
                                        }
                                        BrowserCommand::Login(phone, password) => {
                                            let result = browser.login(&phone, &password).await;
                                            sender.input(AppMsg::LoginCompleted(result));
                                        }
                                        BrowserCommand::GetPayouts => {
                                            let result = browser.get_payouts().await;
                                            sender.input(AppMsg::PayoutsReceived(result));
                                        }
                                        BrowserCommand::GetBalance => {
                                            let result = browser.get_aviator_balance().await;
                                            sender.input(AppMsg::AviatorBalanceReceived(result));
                                        }
                                        BrowserCommand::PlaceBet(amount, odd, mult) => {
                                            let result = browser.place_bet(&amount, &odd, &mult).await;
                                            sender.input(AppMsg::BetResult(result));
                                        }
                                        BrowserCommand::ClickElement(selector) => {
                                            let result = browser.click_by_selector(&selector).await;
                                            sender.input(AppMsg::ScriptExecuted(
                                                result.unwrap_or_else(|e| e)
                                            ));
                                        }
                                        BrowserCommand::ExecuteJs(script) => {
                                            let result = browser.execute_js(&script).await;
                                            sender.input(AppMsg::ScriptExecuted(
                                                result.unwrap_or_else(|e| e)
                                            ));
                                        }
                                        BrowserCommand::SwitchToIframe(index) => {
                                            let result = browser.switch_to_iframe(index).await;
                                            sender.input(AppMsg::UpdateStatus(
                                                result.unwrap_or_else(|e| format!("Iframe error: {}", e))
                                            ));
                                        }
                                        BrowserCommand::GetGameState => {
                                            let result = browser.get_game_state().await;
                                            sender.input(AppMsg::GameStateReceived(result));
                                        }
                                        BrowserCommand::AutoBet(amount, odd, mult) => {
                                            let result = browser.auto_bet(&amount, &odd, &mult).await;
                                            sender.input(AppMsg::AutoBetPlaced(result));
                                        }
                                        BrowserCommand::AutoCashout(target) => {
                                            let result = browser.auto_cashout(&target).await;
                                            sender.input(AppMsg::AutoCashoutResult(result));
                                        }
                                        BrowserCommand::Quit => {
                                            let _ = browser.quit().await;
                                        }
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            sender_clone.input(AppMsg::BrowserStarted(Err(e.to_string())));
                        }
                    }
                });

                // Spawn event receiver
                let sender_events = sender.clone();
                bridge.spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        sender_events.input(AppMsg::BrowserEventReceived(event));
                    }
                });
            }

            AppMsg::BrowserStarted(result) => {
                match result {
                    Ok(msg) => {
                        self.add_log(&format!("[BROWSER] {}", msg));
                        self.status_message = format!("{}", msg);
                        let url = self.site.url.clone();
                        if let Some(bridge) = &self.async_bridge {
                            bridge.send_browser_cmd(BrowserCommand::Navigate(url));
                            self.add_log(&format!("[NAVIGATE] -> {}", self.site.url));
                        }
                    }
                    Err(e) => {
                        self.add_log(&format!("[ERROR] Failed to start browser: {}", e));
                        self.status_message = format!("Failed to start browser: {}", e);
                    }
                }
            }

            AppMsg::StopBrowser => {
                self.add_log("[BROWSER] Stopping browser...");
                if let Some(bridge) = &self.async_bridge {
                    bridge.send_browser_cmd(BrowserCommand::Quit);
                }
                self.async_bridge = None;
                self.auto_play_active = false;
                self.game_phase = GamePhase::Idle;
                self.status_message = "Browser stopped".to_string();
                self.add_log("[BROWSER] Browser stopped");
            }

            AppMsg::NavigateTo(url) => {
                self.add_log(&format!("[NAVIGATE] -> {}", url));
                self.last_loaded_url = url.clone();
                if let Some(bridge) = &self.async_bridge {
                    bridge.send_browser_cmd(BrowserCommand::Navigate(url));
                }
            }

            AppMsg::NavigateComplete(result) => {
                match result {
                    Ok(url) => {
                        self.add_log(&format!("[NAVIGATE] Loaded: {}", url));
                        self.status_message = format!("Loaded: {}", url);
                        self.state = AppState::SiteLoaded;
                        sender.input(AppMsg::InjectDomApi);
                    }
                    Err(e) => {
                        self.add_log(&format!("[ERROR] Navigation failed: {}", e));
                        self.status_message = format!("Navigation failed: {}", e);
                    }
                }
            }

            // ============================================
            // LOGIN
            // ============================================
            AppMsg::PerformLogin => {
                if self.phone.is_empty() || self.password.is_empty() {
                    self.add_log("[LOGIN] ERROR: Phone or password empty");
                    self.status_message = "Enter phone and password".to_string();
                    return;
                }

                self.state = AppState::LoginInProgress;
                self.status_message = "Logging in...".to_string();
                self.add_log(&format!("[LOGIN] Initiating login for phone: {}", self.phone));

                self.send_browser_cmd_queued(BrowserCommand::Login(
                    self.phone.clone(),
                    self.password.clone()
                ));
            }

            AppMsg::ExecuteLogin(phone, password) => {
                self.add_log(&format!("[LOGIN] ExecuteLogin called for phone: {}", phone));
                self.send_browser_cmd_queued(BrowserCommand::Login(phone, password));
            }

            AppMsg::LoginCompleted(result) => {
                match result {
                    Ok(msg) => {
                        self.add_log(&format!("[LOGIN] SUCCESS: {}", msg));
                        self.login_status = LoginStatus::LoggedIn;
                        self.state = AppState::SiteLoaded;
                        self.status_message = format!("{}", msg);
                        self.save_current_state();

                        let sender_nav = sender.clone();
                        glib::timeout_add_local_once(Duration::from_secs(2), move || {
                            sender_nav.input(AppMsg::NavigateToAviator);
                        });
                    }
                    Err(e) => {
                        self.add_log(&format!("[LOGIN] FAILED: {}", e));
                        self.login_status = LoginStatus::LoginFailed(e.clone());
                        self.state = AppState::LoginDialog;
                        self.status_message = format!("{}", e);
                    }
                }
            }

            // ============================================
            // AVIATOR NAVIGATION
            // ============================================
            AppMsg::NavigateToAviator => {
                self.add_log("[AVIATOR] Navigating to Aviator...");
                self.status_message = "Navigating to Aviator...".to_string();

                if let Some(bridge) = &self.async_bridge {
                    let url = "https://www.betika.com/en-ke/aviator".to_string();
                    bridge.send_browser_cmd(BrowserCommand::Navigate(url));
                }
            }

            AppMsg::AviatorNavigated(result) => {
                match result {
                    Ok(_) => {
                        self.add_log("[AVIATOR] Page loaded successfully");
                        self.status_message = "Aviator loaded!".to_string();
                        if let Some(bridge) = &self.async_bridge {
                            bridge.send_browser_cmd(BrowserCommand::SwitchToIframe(2));
                            self.add_log("[AVIATOR] Switching to iframe index 2");
                        }
                        sender.input(AppMsg::CheckAviatorReady);
                    }
                    Err(e) => {
                        self.add_log(&format!("[AVIATOR] Navigation error: {}", e));
                        self.status_message = format!("{}", e);
                    }
                }
            }

            AppMsg::CheckAviatorReady => {
                self.add_log("[AVIATOR] Checking readiness...");
                self.status_message = "Checking Aviator readiness...".to_string();
                let sender_poll = sender.clone();
                glib::timeout_add_local_once(Duration::from_secs(3), move || {
                    sender_poll.input(AppMsg::FetchPayouts);
                });
            }

            AppMsg::AviatorReady => {
                self.add_log("[AVIATOR] Ready! Fetching balance...");
                self.status_message = "Aviator ready! Fetching balance...".to_string();
                sender.input(AppMsg::GetAviatorBalance);
            }

            AppMsg::AviatorPageDetected => {
                self.add_log("[AVIATOR] Page detected");
                self.status_message = "Aviator page detected".to_string();
            }

            // ============================================
            // PAYOUTS
            // ============================================
            AppMsg::FetchPayouts => {
                self.add_log("[PAYOUTS] Fetching payouts...");
                self.send_browser_cmd_queued(BrowserCommand::GetPayouts);
            }

            AppMsg::PayoutsReceived(result) => {
                self.browser_busy = false;
                self.drain_command_queue();
                match result {
                    Ok(payouts) => {
                        self.payouts = payouts.clone();
                        let latest = payouts.last().cloned().unwrap_or_default();
                        self.status_message = format!("{} payouts | Latest: {}", payouts.len(), latest);
                        self.add_log(&format!("[PAYOUTS] Received {} payouts, latest: {}", payouts.len(), latest));
                    }
                    Err(e) => {
                        self.add_log(&format!("[PAYOUTS] Error: {}", e));
                        self.status_message = format!("Payout fetch: {}", e);
                    }
                }
            }

            AppMsg::StartPayoutWatcher => {
                self.payout_watcher_active = true;
                self.status_message = "Watching for new payouts...".to_string();
                self.add_log("[WATCHER] Payout watcher STARTED");
            }

            AppMsg::StopPayoutWatcher => {
                self.payout_watcher_active = false;
                self.status_message = "Payout watcher stopped".to_string();
                self.add_log("[WATCHER] Payout watcher STOPPED");
            }

            AppMsg::NewPayoutReceived(payout) => {
                if !self.payouts.contains(&payout) {
                    self.payouts.push(payout.clone());
                    self.add_log(&format!("[PAYOUTS] NEW: {}", payout));
                    self.status_message = format!("New payout: {}", payout);
                }
            }

            // ============================================
            // BALANCE
            // ============================================
            AppMsg::GetAviatorBalance => {
                if !self.game_phase.can_fetch_balance() {
                    self.add_log("[BALANCE] Skipped — game in progress");
                    return;
                }
                self.add_log("[BALANCE] Fetching balance...");
                self.send_browser_cmd_queued(BrowserCommand::GetBalance);
            }

            AppMsg::AviatorBalanceReceived(result) => {
                self.browser_busy = false;
                self.drain_command_queue();
                match result {
                    Ok(balance) => {
                        if balance.success {
                            let bal = balance.balance.clone().unwrap_or_else(|| "0.00".to_string());
                            self.current_balance = bal.clone();
                            self.status_message = format!("KES {} ({})", bal,
                                balance.source.clone().unwrap_or_else(|| "unknown".to_string()));
                            self.add_log(&format!("[BALANCE] KES {} (source: {})", bal,
                                balance.source.clone().unwrap_or_else(|| "unknown".to_string())));
                        } else {
                            self.status_message = format!("Balance: {}",
                                balance.error.clone().unwrap_or_else(|| "Unknown".to_string()));
                            self.add_log(&format!("[BALANCE] Error: {}",
                                balance.error.clone().unwrap_or_else(|| "Unknown".to_string())));
                        }
                    }
                    Err(e) => {
                        self.add_log(&format!("[BALANCE] Error: {}", e));
                        self.status_message = format!("Balance error: {}", e);
                    }
                }
            }

            AppMsg::PollAviatorBalance => {
                sender.input(AppMsg::GetAviatorBalance);
            }

            // ============================================
            // ALGO STRATEGY
            // ============================================
            AppMsg::FetchAlgoStrategy => {
                let start = self.algo_start.parse::<usize>().unwrap_or(5);
                let end = self.algo_end.parse::<usize>().unwrap_or(1000);
                let profitier = self.algo_profitier.parse::<usize>().unwrap_or(5);
                let multiplier = self.algo_multiplier.parse::<f64>().unwrap_or(1.9);
                let item_no = self.algo_item_no.parse::<usize>().unwrap_or(13).min(13);
                let choice = self.algo_choice;

                self.add_log(&format!(
                    "[ALGO] Generating strategy: start={}, end={}, profitier={}, multiplier={}, item_no={}, choice={}",
                    start, end, profitier, multiplier, item_no, choice
                ));

                let strategy = algo::algo::init(start, end, profitier, multiplier, item_no, choice);
                let tuples = strategy.0;
                let expected_bal = strategy.1;

                let mut result_str = String::new();
                for (i, (odd, mult)) in tuples.iter().enumerate() {
                    result_str.push_str(&format!("{}. ({:.2}, {:.2}) ", i + 1, odd, mult));
                }
                self.algo_strategy = result_str;

                let algo = AlgoStrategy {
                    tuples: tuples.clone(),
                    expected_amount: format!("{:.2}", expected_bal),
                    created_at: SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                };
                self.algo_strategy_data = Some(algo.clone());
                save_algo_strategy(&algo);

                if let Some((odd, mult)) = tuples.first() {
                    self.current_odd = *odd;
                    self.current_multiplier = *mult;
                    self.cashout_target = multiply_rounded(*odd, *mult, 2);
                    self.bet_amount = format!("{:.2}", *odd);
                    self.add_log(&format!(
                        "[ALGO] Strategy loaded: {} tuples | Expected: {:.2} | First tuple: odd={:.2}, mult={:.2}, cashout={:.2} | bet_amount={}",
                        tuples.len(), expected_bal, odd, mult, self.cashout_target, self.bet_amount
                    ));
                }

                self.status_message = format!("Strategy loaded: {} tuples | Expected: {:.2}", tuples.len(), expected_bal);
            }

            AppMsg::AlgoStartChanged(v) => { self.algo_start = v; }
            AppMsg::AlgoEndChanged(v) => { self.algo_end = v; }
            AppMsg::AlgoProfitierChanged(v) => { self.algo_profitier = v; }
            AppMsg::AlgoMultiplierChanged(v) => { self.algo_multiplier = v; }
            AppMsg::AlgoItemNoChanged(v) => { self.algo_item_no = v; }
            AppMsg::AlgoChoiceChanged(v) => { self.algo_choice = v; }

            // ============================================
            // AUTO PLAY — STATE MACHINE DRIVEN
            // ============================================
            AppMsg::StartAutoPlay => {
                if self.algo_strategy_data.is_none() {
                    self.add_log("[AUTO] ERROR: No strategy loaded. Fetch strategy first.");
                    self.status_message = "Load strategy before starting auto play".to_string();
                    return;
                }
                if self.game_phase.is_busy() {
                    self.add_log("[AUTO] Cannot start — browser busy");
                    return;
                }

                self.auto_play_active = true;
                self.game_phase = GamePhase::Idle;
                self.add_log("[AUTO] Auto play STARTED");
                self.status_message = "Auto play active".to_string();

                self.load_next_strategy_tuple();

                // Schedule first tick (one-shot)
                self.schedule_auto_tick(&sender, 1000);
            }

            AppMsg::StopAutoPlay => {
                self.auto_play_active = false;
                self.next_tick_scheduled = false;
                self.game_phase = GamePhase::Idle;
                self.add_log("[AUTO] Auto play STOPPED");
                self.status_message = "Auto play stopped".to_string();
            }

            AppMsg::AutoPlayTick => {
                self.next_tick_scheduled = false;

                if !self.auto_play_active {
                    return;
                }

                // Gate: skip if browser is processing another command
                if self.browser_busy {
                    self.add_log("[AUTO] Tick skipped — browser busy");
                    self.schedule_auto_tick(&sender, 500);
                    return;
                }

                // Gate: skip if in a phase that shouldn't poll
                if self.game_phase == GamePhase::CashoutPending {
                    self.add_log("[AUTO] Tick skipped — cashout pending");
                    self.schedule_auto_tick(&sender, 500);
                    return;
                }

                self.add_log("[AUTO] Tick - fetching game state");
                self.send_browser_cmd_queued(BrowserCommand::GetGameState);
                // Don't schedule next tick here — wait for GameStateReceived
            }

            AppMsg::GameStateReceived(result) => {
                if !self.auto_play_active {
                    self.browser_busy = false;
                    self.drain_command_queue();
                    return;
                }

                self.browser_busy = false;
                self.drain_command_queue();

                match result {
                    Ok(state) => {
                        self.handle_game_state_driven(&state, &sender);
                    }
                    Err(e) => {
                        self.add_log(&format!("[AUTO] Game state ERROR: {}", e));
                        self.game_phase = GamePhase::Error(e.clone());
                        self.schedule_auto_tick(&sender, 2000);
                    }
                }
            }

            AppMsg::AutoBetPlaced(result) => {
                self.browser_busy = false;
                self.drain_command_queue();

                match result {
                    Ok(msg) => {
                        self.game_phase = GamePhase::WaitingRound;
                        self.add_log(&format!("[AUTO] {}", msg));
                        self.status_message = msg.clone();

                        let round = GameRound {
                            round_id: format!("round_{}", self.now_secs()),
                            timestamp: self.now_secs(),
                            odd_used: self.current_odd,
                            multiplier_used: self.current_multiplier,
                            bet_amount: self.bet_amount.parse().unwrap_or(100.0),
                            cashout_target: self.cashout_target,
                            actual_cashout: None,
                            result: RoundResult::Pending,
                            balance_before: self.current_balance.clone(),
                            balance_after: None,
                        };
                        self.pending_round = Some(round);
                        self.add_log("[AUTO] Round started, waiting for plane...");

                        // 🔴 CRITICAL FIX: Schedule next tick to monitor round state
                        self.schedule_auto_tick(&sender, 500);
                    }
                    Err(e) => {
                        self.game_phase = GamePhase::Idle;
                        self.add_log(&format!("[AUTO] Bet failed: {}", e));
                        self.status_message = format!("Auto-bet failed: {}", e);
                        self.schedule_auto_tick(&sender, 2000);
                    }
                }
            }
            AppMsg::AutoCashoutResult(result) => {
                let _was_pending = self.pending_cashout_click;
                self.pending_cashout_click = false;
                self.browser_busy = false;
                self.drain_command_queue();

                match result {
                    Ok(msg) => {
                        if msg.contains("cashed_out") {
                            let parts: Vec<&str> = msg.split(':').collect();
                            let amount = parts.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                            self.add_log(&format!("[AUTO] CASHOUT SUCCESS: {:.2}", amount));
                            self.status_message = format!("Cashed out: {:.2}", amount);

                            if let Some(mut round) = self.pending_round.take() {
                                round.actual_cashout = Some(amount);
                                round.result = RoundResult::Win;
                                round.balance_after = Some(self.current_balance.clone());
                                self.game_history.add_round(round);
                                save_game_history(&self.game_history);
                                self.game_history.reset_strategy();
                                self.add_log("[AUTO] WIN! Reset to first strategy tuple");
                            }

                            self.game_phase = GamePhase::Settling;
                            // Fetch balance after win, then return to Idle
                            self.send_browser_cmd_queued(BrowserCommand::GetBalance);

                            let sender_clone = sender.clone();
                            glib::timeout_add_local_once(Duration::from_secs(2), move || {
                                sender_clone.input(AppMsg::AutoPlayTick);
                            });
                        } else if msg.contains("waiting") {
                            self.game_phase = GamePhase::Flying;
                            self.add_log(&format!("[AUTO] Cashout still waiting: {}", msg));
                            self.schedule_auto_tick(&sender, 300);
                        } else {
                            self.add_log(&format!("[AUTO] Cashout status: {}", msg));
                            self.schedule_auto_tick(&sender, 500);
                        }
                    }
                    Err(e) => {
                        self.add_log(&format!("[AUTO] Cashout error: {}", e));
                        self.game_phase = GamePhase::Idle;

                        if self.pending_round.is_some() {
                            self.add_log("[AUTO] Cashout command failed — treating as LOSS");
                            if let Some(mut round) = self.pending_round.take() {
                                round.result = RoundResult::Loss;
                                round.balance_after = Some(self.current_balance.clone());
                                self.game_history.add_round(round);
                                save_game_history(&self.game_history);

                                if let Some(strategy) = &self.algo_strategy_data {
                                    self.game_history.advance_strategy(strategy.len());
                                    self.load_next_strategy_tuple();
                                }
                            }
                        }
                        self.schedule_auto_tick(&sender, 2000);
                    }
                }
            }

            // ============================================
            // BROWSER EVENTS (event-driven from BrowserController)
            // ============================================
            AppMsg::BrowserEventReceived(event) => {
                match event {
                    BrowserEvent::PayoutChanged(payouts) => {
                        if self.payout_watcher_active {
                            self.payouts = payouts.clone();
                            let latest = payouts.last().cloned().unwrap_or_default();
                            self.status_message = format!("{} payouts | Latest: {}", payouts.len(), latest);
                            self.add_log(&format!("[EVENT] Payouts changed, latest: {}", latest));
                        }
                    }
                    BrowserEvent::BalanceChanged(balance) => {
                        self.current_balance = balance.clone();
                        self.status_message = format!("KES {}", balance);
                        self.add_log(&format!("[EVENT] Balance updated: KES {}", balance));
                    }
                    BrowserEvent::GameStateChanged(state) => {
                        if self.auto_play_active {
                            self.handle_game_state_driven(&state, &sender);
                        }
                    }
                    BrowserEvent::BetConfirmed => {
                        self.add_log("[EVENT] Bet confirmed by browser");
                    }
                    BrowserEvent::CashoutConfirmed(amount) => {
                        self.add_log(&format!("[EVENT] Cashout confirmed: {:.2}", amount));
                    }
                    BrowserEvent::RoundCrashed => {
                        self.add_log("[EVENT] Round crashed");
                        if self.auto_play_active && self.pending_round.is_some() {
                            self.record_loss();
                            self.game_phase = GamePhase::Idle;
                            self.schedule_auto_tick(&sender, 1500);
                        }
                    }
                    BrowserEvent::Error(e) => {
                        self.add_log(&format!("[EVENT] Browser error: {}", e));
                    }
                    BrowserEvent::NetworkStatus(online) => {
                        sender.input(AppMsg::NetworkStatusChanged(online));
                    }
                }
            }

            // ============================================
            // MANUAL BETTING
            // ============================================
            AppMsg::PlaceBet(_, _, _) => {
                let amount = self.bet_amount.clone();
                self.add_log(&format!("[BET] Placing bet: KES {}", amount));
                self.send_browser_cmd_queued(BrowserCommand::PlaceBet(
                    amount,
                    "2.0".to_string(),
                    "1.5".to_string()
                ));
            }

            AppMsg::Cashout => {
                self.add_log("[BET] Cashout requested");
                let script = r#"
                    (function() {
                        var btn = document.querySelector('.cashout-button, [data-testid="cashout"]');
                        if (btn) { btn.click(); return 'cashout_clicked'; }
                        return 'cashout_not_found';
                    })();
                "#.to_string();

                self.send_browser_cmd_queued(BrowserCommand::ExecuteJs(script));
            }

            AppMsg::BetResult(result) => {
                self.browser_busy = false;
                self.drain_command_queue();
                match result {
                    Ok(msg) => {
                        self.status_message = format!("Bet: {}", msg);
                        self.add_log(&format!("[BET] Result: {}", msg));
                    }
                    Err(e) => {
                        self.status_message = format!("Bet failed: {}", e);
                        self.add_log(&format!("[BET] Failed: {}", e));
                    }
                }
            }

            // ============================================
            // DEMO MODE
            // ============================================
            AppMsg::ClickAviatorDemo => {
                if self.login_status == LoginStatus::LoggedIn {
                    self.status_message = "You are logged in already".to_string();
                    self.add_log("[DEMO] User already logged in");
                    return;
                }

                self.is_demo_mode = true;
                self.add_log("[DEMO] Navigating to Aviator demo...");
                self.status_message = "Navigating to Aviator demo...".to_string();

                if let Some(bridge) = &self.async_bridge {
                    let url = "https://www.betika.com/en-ke/aviator".to_string();
                    bridge.send_browser_cmd(BrowserCommand::Navigate(url));
                }

                let sender_demo = sender.clone();
                glib::timeout_add_local_once(Duration::from_secs(5), move || {
                    sender_demo.input(AppMsg::ClickDemoButton);
                });
            }

            AppMsg::ClickDemoButton => {
                self.add_log("[DEMO] Clicking 'Click to play demo' button...");

                let script = r#"
                (function() {
                    var btn = document.querySelector('button.account__payments__submit.button__secondary.purple');
                    if (btn) {
                        btn.scrollIntoView({behavior: 'smooth', block: 'center'});
                        btn.click();
                        btn.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
                        return 'demo_button_clicked';
                    }
                    var buttons = document.querySelectorAll('button');
                    for (var i = 0; i < buttons.length; i++) {
                        var text = buttons[i].textContent.toLowerCase().trim();
                        if (text.includes('demo') || text.includes('play demo')) {
                            buttons[i].scrollIntoView({behavior: 'smooth', block: 'center'});
                            buttons[i].click();
                            return 'demo_button_found_by_text';
                        }
                    }
                    return 'demo_button_not_found';
                })();
                "#;

                self.send_browser_cmd_queued(BrowserCommand::ExecuteJs(script.to_string()));
            }

            AppMsg::AviatorDemoResult(result) => {
                match result {
                    Ok(msg) => {
                        self.status_message = msg.clone();
                        self.add_log(&format!("[AVIATOR] Demo: {}", msg));
                    }
                    Err(e) => {
                        self.status_message = format!("{}", e);
                        self.add_log(&format!("[AVIATOR] Demo error: {}", e));
                    }
                }
            }

            // ============================================
            // DOM / SESSION
            // ============================================
            AppMsg::InjectDomApi => {
                self.add_log("[DOM] Injecting DOM helpers...");
                self.send_browser_cmd_queued(BrowserCommand::ExecuteJs(
                    DOM_HELPERS_SCRIPT.to_string()
                ));
                self.status_message = "DOM API injected".to_string();
                self.add_log("[DOM] DOM API injected");
            }

            AppMsg::DomApiReady(ready) => {
                self.dom_api_ready = ready;
                self.add_log(&format!("[DOM] API ready: {}", ready));
            }

            AppMsg::CheckDomApiReady => {
                let script = "(typeof window.__aviet !== 'undefined') ? 'ready' : 'missing';".to_string();
                self.send_browser_cmd_queued(BrowserCommand::ExecuteJs(script));
            }

            AppMsg::RestoreSession => {
                self.add_log("[SESSION] Restoring session...");
                self.session_restored = true;
                self.login_status = LoginStatus::LoggedIn;
                self.status_message = "Restoring session...".to_string();

                if let Some(bridge) = &self.async_bridge {
                    bridge.send_browser_cmd(BrowserCommand::Navigate(self.last_loaded_url.clone()));
                }
            }

            AppMsg::ValidateSession => {
                let script = "JSON.stringify(window.__aviet.getAuthState());".to_string();
                self.send_browser_cmd_queued(BrowserCommand::ExecuteJs(script));
            }

            AppMsg::SessionValid(valid) => {
                if valid {
                    self.login_status = LoginStatus::LoggedIn;
                    self.add_log("[SESSION] Session validated - logged in");
                }
            }

            AppMsg::GetSessionInfo => {}
            AppMsg::SessionInfoReceived(_) => {}
            AppMsg::SaveSessionToStorage(_) => {}
            AppMsg::LoadSessionFromStorage => {}
            AppMsg::SessionLoadedFromStorage(_) => {}

            // ============================================
            // ALGO
            // ============================================
            AppMsg::AlgoStrategyReceived(result) => {
                match result {
                    Ok(strategy) => {
                        let mut s = String::new();
                        for (a, b) in strategy {
                            s.push_str(&format!("({:.0}, {:.0}) ", a, b));
                        }
                        self.algo_strategy = s;
                    }
                    Err(e) => {
                        self.status_message = format!("Algo error: {}", e);
                        self.add_log(&format!("[ALGO] Error: {}", e));
                    }
                }
            }

            AppMsg::GetGameStrategy(_, _, _, _, _) => {}

            // ============================================
            // INPUT HANDLERS
            // ============================================
            AppMsg::PhoneChanged(phone) => { self.phone = phone; }
            AppMsg::PasswordChanged(password) => { self.password = password; }
            AppMsg::SaveCredentials(save) => { self.save_credentials = save; }
            AppMsg::BetAmountChanged(amount) => { self.bet_amount = amount; }

            AppMsg::SelectSite(site) => {
                self.site = site.clone();
                self.last_loaded_url = site.url.clone();
                self.add_log(&format!("[SITE] Selected: {} -> {}", site.name, site.url));
            }

            AppMsg::ToggleTheme => {
                self.is_dark_mode = !self.is_dark_mode;
                if let Some(settings) = gtk::Settings::default() {
                    settings.set_gtk_application_prefer_dark_theme(self.is_dark_mode);
                }
                self.save_current_state();
                self.add_log(&format!("[THEME] Switched to {}", if self.is_dark_mode { "dark" } else { "light" }));
            }

            AppMsg::RefreshPage => {
                self.add_log(&format!("[NAVIGATE] Refreshing: {}", self.last_loaded_url));
                self.send_browser_cmd_queued(BrowserCommand::Navigate(self.last_loaded_url.clone()));
            }

            AppMsg::CloseSite => {
                self.add_log("[SITE] Closing site...");
                self.state = AppState::SiteSelection;
                self.login_status = LoginStatus::Unknown;
                self.status_message = "Ready".to_string();
                self.session_restored = false;
                self.is_demo_mode = false;
                self.auto_play_active = false;
                self.game_phase = GamePhase::Idle;
                if let Some(bridge) = &self.async_bridge {
                    bridge.send_browser_cmd(BrowserCommand::Navigate("about:blank".to_string()));
                }
                clear_state();
            }

            // ============================================
            // NETWORK — STATE GATED
            // ============================================
            AppMsg::CheckNetwork => {
                if self.network_monitor.is_checking || self.game_phase.is_busy() {
                    return;
                }
                self.network_monitor.is_checking = true;

                let sender_net = sender.clone();
                if let Some(bridge) = &self.async_bridge {
                    let bridge = bridge.clone();
                    bridge.spawn(async move {
                        let client = reqwest::Client::builder()
                            .timeout(Duration::from_secs(3))
                            .build()
                            .unwrap();
                        let online = client.get("https://www.google.com").send().await.is_ok();
                        sender_net.input(AppMsg::NetworkStatusChanged(online));
                    });
                }
            }

            AppMsg::NetworkStatusChanged(online) => {
                self.network_monitor.set_online(online);
                self.network_monitor.is_checking = false;

                if !online {
                    if self.auto_play_active {
                        self.add_log("[NETWORK] OFFLINE — auto-play will resume when back");
                    }
                    self.state = AppState::Offline;
                    self.status_message = "Offline".to_string();
                    self.add_log("[NETWORK] OFFLINE");
                } else if self.state == AppState::Offline {
                    self.state = AppState::SiteLoaded;
                    self.status_message = "Back online".to_string();
                    self.add_log("[NETWORK] Back ONLINE");
                    if self.auto_play_active && !self.next_tick_scheduled {
                        self.schedule_auto_tick(&sender, 1000);
                    }
                }
            }

            AppMsg::RetryConnection => {
                self.add_log("[NETWORK] Retrying connection...");
                sender.input(AppMsg::CheckNetwork);
            }

            AppMsg::PageLoadError(error) => {
                self.status_message = format!("Page error: {}", error);
                self.add_log(&format!("[ERROR] Page load: {}", error));
            }

            AppMsg::UpdateStatus(msg) => {
                if msg == "State saved" {
                    self.save_current_state();
                }
                self.status_message = msg.clone();
                self.add_log(&format!("[STATUS] {}", msg));
            }

            AppMsg::ScriptExecuted(result) => {
                self.browser_busy = false;
                self.drain_command_queue();
                self.status_message = format!("{}", result);
                self.add_log(&format!("[SCRIPT] {}", result));
            }

            AppMsg::CheckBalance => {}
            AppMsg::BalanceChecked(_) => {}
            AppMsg::Deposit(_) => {}
            AppMsg::Withdraw(_) => {}
            AppMsg::InitializeGame => {}
            AppMsg::MonitorGame => {}
            AppMsg::PauseGame => {}
            AppMsg::StopGame => {}
            AppMsg::ResumePlay(_, _) => {}
            AppMsg::LoadPlayHistory => {}
            AppMsg::SavePlayHistory => {}
            AppMsg::ClearPlayHistory => {}
            AppMsg::GetRoundHistory => {}
            AppMsg::SaveRoundHistory => {}

            AppMsg::LogMessage(msg) => {
                self.add_log(&msg);
            }

            AppMsg::ClearLogs => {
                self.logs.clear();
                self.log_counter = 0;
                self.add_log("[LOGS] Cleared");
            }

            AppMsg::TogglePayoutWatcher => {}
            AppMsg::StrategyTupleUsed(_) => {}
            AppMsg::RoundCompleted { .. } => {}
            AppMsg::CheckWinReflected => {}
            AppMsg::WinReflected(_) => {}
        }
    }
}

    // ============================================
    // APP METHODS
    // ============================================

    impl App {
        // --- One-shot timer with deduplication ---
        fn schedule_auto_tick(&mut self, sender: &ComponentSender<Self>, delay_ms: u64) {
            if self.next_tick_scheduled {
                return;
            }
            self.next_tick_scheduled = true;

            let sender = sender.clone();
            glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
                sender.input(AppMsg::AutoPlayTick);
            });
        }

        // --- Command queue with backpressure ---
        fn send_browser_cmd_queued(&mut self, cmd: BrowserCommand) {
            if self.browser_busy {
                self.command_queue.push(cmd);
                self.add_log(&format!("[QUEUE] Command queued ({} total)", self.command_queue.len()));
                return;
            }
            self.browser_busy = true;
            if let Some(bridge) = &self.async_bridge {
                bridge.send_browser_cmd(cmd);
            }
        }

        fn drain_command_queue(&mut self) {
            if let Some(cmd) = self.command_queue.pop() {
                self.add_log("[QUEUE] Draining queued command");
                self.browser_busy = true;
                if let Some(bridge) = &self.async_bridge {
                    bridge.send_browser_cmd(cmd);
                }
            }
        }

        // --- Strategy tuple loader ---
        fn load_next_strategy_tuple(&mut self) {
            if let Some(strategy) = &self.algo_strategy_data {
                let idx = self.game_history.current_strategy_index;
                if let Some((odd, mult)) = strategy.get_current_tuple(idx) {
                    self.current_odd = odd;
                    self.current_multiplier = mult;
                    self.cashout_target = multiply_rounded(odd, mult, 2);
                    self.bet_amount = format!("{:.2}", odd);
                    self.add_log(&format!(
                        "[AUTO] Using tuple {}: odd={:.2}, mult={:.2}, target={:.2}, bet_amount={}",
                        idx + 1, odd, mult, self.cashout_target, self.bet_amount
                    ));
                }
            }
        }

        fn record_loss(&mut self) {
            self.add_log("[AUTO] Recording LOSS");
            if let Some(mut round) = self.pending_round.take() {
                round.result = RoundResult::Loss;
                round.balance_after = Some(self.current_balance.clone());
                self.game_history.add_round(round);
                save_game_history(&self.game_history);

                if let Some(strategy) = &self.algo_strategy_data {
                    self.game_history.advance_strategy(strategy.len());
                    self.load_next_strategy_tuple();
                }
            }
        }

        fn place_autobet(&mut self, sender: &ComponentSender<Self>) {
            let now = self.now_secs();
            if now - self.last_bet_timestamp < 5 {
                self.add_log(&format!(
                    "[AUTO] Bet cooldown: {}s remaining",
                    5 - (now - self.last_bet_timestamp)
                ));
                self.schedule_auto_tick(sender, 1000);
                return;
            }

            let amount = self.bet_amount.clone();
            let odd = format!("{:.2}", self.current_odd);
            let mult = format!("{:.2}", self.current_multiplier);

            self.add_log(&format!(
                "[AUTO] READY TO BET - Placing: KES {} @ odd={}x mult={} target={:.2}",
                amount, odd, mult, self.cashout_target
            ));

            self.last_bet_timestamp = now;
            self.game_phase = GamePhase::Betting;
            self.send_browser_cmd_queued(BrowserCommand::AutoBet(amount, odd, mult));
        }

        // --- State machine driven game state handler ---
        fn handle_game_state_driven(&mut self, state: &GameState, sender: &ComponentSender<Self>) {
            use GamePhase::*;

            self.add_log(&format!(
                "[AUTO] Phase={:?} | class='{}' text='{}' waiting={} cashout={} amount={:?}",
                self.game_phase,
                state.bet_button_class,
                state.bet_button_text,
                state.is_waiting,
                state.is_cashout_available,
                state.cashout_amount
            ));

            match &self.game_phase {
                Idle => {
                    if state.bet_button_class.contains("btn-success") && state.bet_button_class.contains("bet") {
                        self.place_autobet(sender);
                    } else {
                        self.schedule_auto_tick(sender, 500);
                    }
                }

                Betting => {
                    // Waiting for AutoBetPlaced response
                    self.schedule_auto_tick(sender, 300);
                }

                WaitingRound => {
                    // 🔴 TIMEOUT PROTECTION: recover if stuck too long
                    let waiting_too_long = self.pending_round.as_ref().map_or(false, |r| {
                        self.now_secs().saturating_sub(r.timestamp) > 45 // Max 45s per round
                    });

                    if waiting_too_long {
                        self.add_log("[AUTO] TIMEOUT in WaitingRound — round likely crashed instantly");
                        self.record_loss();
                        self.game_phase = Idle;
                        self.schedule_auto_tick(sender, 1000);
                        return;
                    }

                    if state.is_cashout_available {
                        self.game_phase = Flying;
                        self.add_log("[AUTO] → Flying (cashout button visible)");
                        self.schedule_auto_tick(sender, 300);
                    } else if state.bet_button_class.contains("btn-danger") && !state.is_waiting {
                        self.game_phase = Flying;
                        self.add_log("[AUTO] → Flying (cancel button active, no tooltip)");
                        self.schedule_auto_tick(sender, 300);
                    } else if state.bet_button_class.contains("btn-success")
                        && state.bet_button_text.to_lowercase().contains("bet") {
                        self.add_log("[AUTO] → Idle (round ended instantly — back to bet)");
                        self.record_loss();
                        self.game_phase = Idle;
                        self.schedule_auto_tick(sender, 1000);
                    } else {
                        self.schedule_auto_tick(sender, 500);
                    }
                }
                Flying => {
                    if state.is_cashout_available {
                        if let Some(ref amount_str) = state.cashout_amount {
                            if let Ok(current) = self.parse_cashout_amount(amount_str) {
                                self.add_log(&format!(
                                    "[AUTO] CASHOUT PHASE: current={:.2} target={:.2}",
                                    current, self.cashout_target
                                ));

                                if float_eq(current, self.cashout_target, 0.01) || current >= self.cashout_target {
                                    self.add_log(&format!(
                                        "[AUTO] TARGET REACHED! {:.2} >= {:.2} - clicking cashout",
                                        current, self.cashout_target
                                    ));

                                    self.pending_cashout_click = true;
                                    self.cashout_click_time = self.now_secs();
                                    self.game_phase = CashoutPending;

                                    self.send_browser_cmd_queued(BrowserCommand::AutoCashout(
                                        format!("{:.2}", self.cashout_target)
                                    ));
                                    return;
                                }
                            }
                        }
                        self.schedule_auto_tick(sender, 300);
                    } else if state.bet_button_class.contains("btn-success") && state.bet_button_class.contains("bet") {
                        self.add_log("[AUTO] CRASHED before cashout!");
                        self.record_loss();
                        self.game_phase = Idle;
                        self.schedule_auto_tick(sender, 1000);
                    } else {
                        self.schedule_auto_tick(sender, 300);
                    }
                }

                CashoutPending => {
                    self.add_log("[AUTO] Warning: state handler called in CashoutPending");
                }

                Settling => {
                    self.game_phase = Idle;
                    self.schedule_auto_tick(sender, 1000);
                }

                Error(_) => {
                    self.game_phase = Idle;
                    self.schedule_auto_tick(sender, 3000);
                }
            }
        }
        fn parse_cashout_amount(&self, s: &str) -> Result<f64, String> {
            let num_part: String = s.chars().filter(|c: &char| c.is_numeric() || *c == '.').collect();
            num_part.parse().map_err(|_| format!("Cannot parse: '{}'", s))
        }

        fn now_secs(&self) -> u64 {
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
        }

        fn save_current_state(&self) {
            let snapshot = AppStateSnapshot {
                login_status: self.login_status.clone(),
                phone: self.phone.clone(),
                bet_amount: self.bet_amount.clone(),
                last_url: self.last_loaded_url.clone(),
                timestamp: self.now_secs(),
                network_was_up: self.network_monitor.check_online(),
                is_dark_mode: self.is_dark_mode,
            };
            save_state(&snapshot);
        }

        fn add_log(&mut self, msg: &str) {
            let timestamp = self.now_secs();
            let time_str = format!("{:02}:{:02}:{:02}",
                                   (timestamp / 3600) % 24,
                                   (timestamp / 60) % 60,
                                   timestamp % 60
            );
            self.log_counter += 1;
            self.logs.push(format!("[{}] {}", time_str, msg));
            if self.logs.len() > 200 {
                self.logs.remove(0);
            }
        }
    }

fn main() {
    let app = RelmApp::new("AvietGianna");
        app.run::<App>(());
    }