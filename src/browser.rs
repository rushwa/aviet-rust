use thirtyfour::prelude::*;
use thirtyfour::WebDriver;
use std::time::Duration;
use tokio::time::sleep;
use thirtyfour::common::capabilities::firefox::FirefoxPreferences;
use tokio::sync::mpsc;
pub(crate) use crate::models::{AviatorBalance, BrowserEvent, GameState};
use crate::algo::algo::{multiply_rounded, float_eq};

#[derive(Debug, Clone)]
pub enum BrowserCommand {
    Navigate(String),
    Login(String, String),
    GetPayouts,
    GetBalance,
    PlaceBet(String, String, String),
    ClickElement(String),
    ExecuteJs(String),
    SwitchToIframe(usize),
    GetGameState,
    AutoBet(String, String, String),
    AutoCashout(String),
    Quit,
}

pub struct BrowserController {
    driver: WebDriver,
    event_tx: mpsc::UnboundedSender<BrowserEvent>,
}

impl BrowserController {
    pub async fn new(event_tx: mpsc::UnboundedSender<BrowserEvent>) -> Result<Self, anyhow::Error> {
        let mut caps = DesiredCapabilities::firefox();

        let mut prefs = FirefoxPreferences::new();
        prefs.set("dom.webdriver.enabled", false)?;
        prefs.set("useAutomationExtension", false)?;
        prefs.set("general.useragent.override", "Mozilla/5.0 (X11; Linux x86_64; rv:126.0) Gecko/20100101 Firefox/126.0")?;
        prefs.set("intl.accept_languages", "en-US,en")?;
        prefs.set("browser.download.start_downloads_in_tmp_dir", true)?;
        prefs.set("browser.download.folderList", 2)?;
        prefs.set("browser.download.useDownloadDir", true)?;
        prefs.set("browser.download.viewableInternally.enabledTypes", "")?;
        prefs.set("browser.helperApps.neverAsk.saveToDisk", "application/pdf,text/plain")?;
        prefs.set("pdfjs.disabled", true)?;
        prefs.set("browser.tabs.firefox-view", false)?;
        prefs.set("browser.startup.homepage", "about:blank")?;
        prefs.set("browser.startup.page", 0)?;
        prefs.set("browser.sessionstore.resume_from_crash", false)?;
        prefs.set("browser.shell.checkDefaultBrowser", false)?;
        prefs.set("browser.warnOnQuit", false)?;
        prefs.set("browser.tabs.warnOnClose", false)?;
        prefs.set("browser.tabs.warnOnOpen", false)?;
        prefs.set("dom.disable_beforeunload", true)?;
        prefs.set("dom.disable_open_during_load", false)?;
        prefs.set("privacy.popups.disable_popup_notifications", true)?;
        prefs.set("privacy.popups.policy", 1)?;
        prefs.set("network.http.phishy-userpass-length", 255)?;
        prefs.set("security.csp.enable", false)?;
        prefs.set("security.fileuri.strict_origin_policy", false)?;

        caps.set_preferences(prefs)?;

        let args = vec![
            "--width=1480",
            "--height=1080",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-gpu",
            "--disable-software-rasterizer",
        ];

        for arg in args {
            caps.add_arg(arg)?;
        }

        let driver = WebDriver::new("http://localhost:4444", caps).await?;

        let _ = driver.execute(r#"
            Object.defineProperty(navigator, 'webdriver', {
                get: () => undefined
            });
        "#, vec![]).await;

        Ok(Self { driver, event_tx })
    }

    // ============================================
    // NAVIGATION
    // ============================================

    pub async fn navigate(&mut self, url: &str) -> Result<String, String> {
        self.driver.goto(url).await.map_err(|e| e.to_string())?;
        sleep(Duration::from_secs(3)).await;
        let current = self.driver.current_url().await.map_err(|e| e.to_string())?;
        Ok(current.to_string())
    }

    // ============================================
    // LOGIN
    // ============================================

    pub async fn login(&mut self, phone: &str, password: &str) -> Result<String, String> {
        let login_url = "https://www.betika.com/en-ke/login";
        self.driver.goto(login_url).await.map_err(|e| format!("Failed to navigate to login: {}", e))?;
        sleep(Duration::from_secs(3)).await;
        sleep(Duration::from_secs(2)).await;

        let phone_filled = self.fill_phone_input(phone).await?;
        if !phone_filled {
            return Err("Could not find phone input field".to_string());
        }
        sleep(Duration::from_millis(500)).await;

        let pass_filled = self.fill_password_input(password).await?;
        if !pass_filled {
            return Err("Could not find password input field".to_string());
        }
        sleep(Duration::from_millis(500)).await;

        let btn_clicked = self.click_login_button().await?;
        if !btn_clicked {
            return Err("Could not find or click login button".to_string());
        }

        sleep(Duration::from_secs(5)).await;

        let final_url = self.driver.current_url().await.map_err(|e| e.to_string())?;
        let final_url_str = final_url.to_string();

        if final_url_str.contains("login") {
            let error_msg = self.get_login_error().await;
            return Err(format!("Login failed - still on login page. {}", error_msg));
        }

        Ok("Login successful".to_string())
    }

    async fn fill_phone_input(&mut self, phone: &str) -> Result<bool, String> {
        let phone_selectors = vec![
            "input[name='phone-number']",
            "input[name='phone']",
            "input[type='tel']",
            "input[placeholder*='07']",
            "input[placeholder*='phone']",
            "input[id*='phone']",
            "input[autocomplete='tel']",
            "input.input.phone-number",
            "input[formcontrolname='phoneNumber']",
            "input[formcontrolname='phone']",
            "input[aria-label*='phone']",
            "input[aria-label*='Phone']",
        ];

        for selector in phone_selectors {
            if let Ok(input) = self.driver.find(By::Css(selector)).await {
                let _ = self.driver.execute(&format!(
                    "document.querySelector('{}').scrollIntoView({{behavior: 'smooth', block: 'center'}});",
                    selector
                ), vec![]).await;
                sleep(Duration::from_millis(300)).await;
                let _ = input.click().await;
                sleep(Duration::from_millis(200)).await;
                let _ = input.clear().await;
                sleep(Duration::from_millis(200)).await;
                let _ = input.send_keys(phone).await;
                let value = input.prop("value").await.map_err(|e| e.to_string())?;
                if value.as_ref().map(|v| !v.is_empty()).unwrap_or(false) {
                    return Ok(true);
                }
            }
        }

        let phone_xpaths = vec![
            "//input[@name='phone-number']",
            "//input[@type='tel']",
            "//input[contains(@placeholder, '07')]",
            "//input[contains(@placeholder, 'phone')]",
            "//input[contains(@aria-label, 'phone')]",
            "//input[contains(@class, 'phone')]",
        ];

        for xpath in phone_xpaths {
            if let Ok(input) = self.driver.find(By::XPath(xpath)).await {
                let _ = input.click().await;
                sleep(Duration::from_millis(200)).await;
                let _ = input.clear().await;
                sleep(Duration::from_millis(200)).await;
                let _ = input.send_keys(phone).await;
                return Ok(true);
            }
        }

        let js_result = self.driver.execute(&format!(
            r#"
            (function() {{
                var inputs = document.querySelectorAll('input');
                for (var i = 0; i < inputs.length; i++) {{
                    var inp = inputs[i];
                    var type = (inp.type || '').toLowerCase();
                    var name = (inp.name || '').toLowerCase();
                    var placeholder = (inp.placeholder || '').toLowerCase();
                    var cls = (inp.className || '').toLowerCase();
                    if (type === 'tel' || name.includes('phone') || placeholder.includes('phone') ||
                        placeholder.includes('07') || cls.includes('phone')) {{
                        inp.value = '{}';
                        inp.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        inp.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return 'filled';
                    }}
                }}
                return 'not_found';
            }})();
            "#,
            phone.replace("'", "\\'")
        ), vec![]).await.map_err(|e| e.to_string())?;

        let result = js_result.json().to_string();
        Ok(result.contains("filled"))
    }

    async fn fill_password_input(&mut self, password: &str) -> Result<bool, String> {
        let pass_selectors = vec![
            "input[type='password']",
            "input[name='password']",
            "input[id*='password']",
            "input[placeholder*='password']",
            "input[placeholder*='Password']",
            "input[autocomplete='current-password']",
            "input[formcontrolname='password']",
            "input[aria-label*='password']",
            "input[aria-label*='Password']",
        ];

        for selector in pass_selectors {
            if let Ok(input) = self.driver.find(By::Css(selector)).await {
                let _ = self.driver.execute(&format!(
                    "document.querySelector('{}').scrollIntoView({{behavior: 'smooth', block: 'center'}});",
                    selector
                ), vec![]).await;
                sleep(Duration::from_millis(300)).await;
                let _ = input.click().await;
                sleep(Duration::from_millis(200)).await;
                let _ = input.clear().await;
                sleep(Duration::from_millis(200)).await;
                let _ = input.send_keys(password).await;
                let value = input.prop("value").await.map_err(|e| e.to_string())?;
                if value.as_ref().map(|v| !v.is_empty()).unwrap_or(false) {
                    return Ok(true);
                }
            }
        }

        let js_result = self.driver.execute(&format!(
            r#"
            (function() {{
                var inputs = document.querySelectorAll('input[type="password"]');
                if (inputs.length > 0) {{
                    inputs[0].value = '{}';
                    inputs[0].dispatchEvent(new Event('input', {{ bubbles: true }}));
                    inputs[0].dispatchEvent(new Event('change', {{ bubbles: true }}));
                    return 'filled';
                }}
                var allInputs = document.querySelectorAll('input');
                for (var i = 0; i < allInputs.length; i++) {{
                    if (allInputs[i].type === 'password' || allInputs[i].name.toLowerCase().includes('pass')) {{
                        allInputs[i].value = '{}';
                        allInputs[i].dispatchEvent(new Event('input', {{ bubbles: true }}));
                        allInputs[i].dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return 'filled';
                    }}
                }}
                return 'not_found';
            }})();
            "#,
            password.replace("'", "\\'"),
            password.replace("'", "\\'")
        ), vec![]).await.map_err(|e| e.to_string())?;

        let result = js_result.json().to_string();
        Ok(result.contains("filled"))
    }

    async fn click_login_button(&mut self) -> Result<bool, String> {
        let btn_selectors = vec![
            "button[type='submit']",
            "button.login-button",
            "button.btn-login",
            "button[class*='login']",
            "button[class*='submit']",
            "[data-testid='login-button']",
            "[data-testid='submit']",
            "button.primary",
            "button.suggested-action",
            "input[type='submit']",
        ];

        for selector in btn_selectors {
            if let Ok(btn) = self.driver.find(By::Css(selector)).await {
                let _ = self.driver.execute(&format!(
                    "document.querySelector('{}').scrollIntoView({{behavior: 'smooth', block: 'center'}});",
                    selector
                ), vec![]).await;
                sleep(Duration::from_millis(500)).await;
                let click_result = btn.click().await;
                if click_result.is_ok() {
                    sleep(Duration::from_millis(500)).await;
                    return Ok(true);
                }
            }
        }

        let btn_xpaths = vec![
            "//button[contains(text(), 'Login')]",
            "//button[contains(text(), 'Log in')]",
            "//button[contains(text(), 'Sign in')]",
            "//button[contains(text(), 'Submit')]",
            "//input[@type='submit']",
            "//button[@type='submit']",
            "//a[contains(text(), 'Login')]",
            "//span[contains(text(), 'Login')]/parent::button",
            "//span[contains(text(), 'Log in')]/parent::button",
        ];

        for xpath in btn_xpaths {
            if let Ok(btn) = self.driver.find(By::XPath(xpath)).await {
                let _ = btn.click().await;
                sleep(Duration::from_millis(500)).await;
                return Ok(true);
            }
        }

        let js_result = self.driver.execute(r#"
            (function() {
                var buttons = document.querySelectorAll('button, input[type="submit"], a[href*="login"]');
                for (var i = 0; i < buttons.length; i++) {
                    var text = (buttons[i].textContent || buttons[i].value || '').toLowerCase().trim();
                    if (text.includes('login') || text.includes('log in') || text.includes('sign in') ||
                        text.includes('submit') || buttons[i].type === 'submit') {
                        buttons[i].scrollIntoView({behavior: 'smooth', block: 'center'});
                        buttons[i].click();
                        buttons[i].dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
                        if (buttons[i].form) {
                            buttons[i].form.submit();
                        }
                        return 'clicked: ' + text;
                    }
                }
                var forms = document.querySelectorAll('form');
                for (var j = 0; j < forms.length; j++) {
                    if (forms[j].querySelector('input[type="password"]')) {
                        forms[j].submit();
                        return 'form_submitted';
                    }
                }
                return 'not_found';
            })();
        "#, vec![]).await.map_err(|e| e.to_string())?;

        let result = js_result.json().to_string();
        Ok(result.contains("clicked") || result.contains("form_submitted"))
    }

    async fn get_login_error(&mut self) -> String {
        let error_selectors = vec![
            ".error-message",
            ".alert-danger",
            ".login-error",
            "[role='alert']",
            ".notification-error",
        ];

        for selector in error_selectors {
            if let Ok(el) = self.driver.find(By::Css(selector)).await {
                if let Ok(text) = el.text().await {
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
            }
        }
        "No specific error message found".to_string()
    }

    // ============================================
    // AVIATOR IFRAME HELPERS
    // ============================================

    async fn enter_aviator_iframe(&mut self) -> Result<bool, String> {
        let iframes = self.driver.find_all(By::Tag("iframe")).await
            .map_err(|e| e.to_string())?;

        for iframe in &iframes {
            if let Ok(src) = iframe.attr("src").await {
                if let Some(src_str) = src {
                    if src_str.contains("aviator") || src_str.contains("spribe") || src_str.contains("casino") || src_str.contains("game") {
                        iframe.clone().enter_frame().await.map_err(|e| e.to_string())?;
                        sleep(Duration::from_millis(800)).await;
                        return Ok(true);
                    }
                }
            }
        }

        if !iframes.is_empty() {
            for idx in [2usize, 0, 1] {
                if let Some(iframe) = iframes.get(idx) {
                    let _ = iframe.clone().enter_frame().await;
                    sleep(Duration::from_millis(500)).await;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn exit_iframe(&mut self) {
        let _ = self.driver.enter_default_frame().await;
    }

    // ============================================
    // CASHOUT HELPERS (shared, no duplication)
    // ============================================

    async fn find_cashout_button(&self) -> Result<WebElement, String> {
        let first_panel = "app-bet-control:first-of-type";

        for selector in [
            &format!("{} .buttons-block .btn.btn-warning.cashout", first_panel),
            &format!("{} .buttons-block .btn-warning", first_panel),
            &format!("{} .buttons-block .btn", first_panel),
            ".btn-warning.cashout",
            ".btn-warning",
            "[class*='cashout']",
            "[class*='warning']",
        ] {
            if let Ok(btn) = self.driver.find(By::Css(selector)).await {
                let cls = btn.attr("class").await.ok().flatten().unwrap_or_default();
                let text = btn.text().await.unwrap_or_default();

                let is_cashout = cls.contains("btn-warning")
                    || cls.contains("cashout")
                    || text.to_lowercase().contains("cash")
                    || text.to_lowercase().contains("out");

                if is_cashout {
                    return Ok(btn);
                }
            }
        }

        if let Ok(all_btns) = self.driver.find_all(By::Css("button")).await {
            for btn in all_btns {
                let text = btn.text().await.unwrap_or_default().to_lowercase();
                if text.contains("cash") || text.contains("out") {
                    return Ok(btn);
                }
            }
        }

        Err("Cashout button not found".to_string())
    }

    async fn get_cashout_amount(&self, btn: &WebElement) -> Result<f64, String> {
        let amount_text = if let Ok(span) = btn.find(By::Css("label.amount span")).await {
            span.text().await.ok()
        } else if let Ok(span) = btn.find(By::Css(".amount span")).await {
            span.text().await.ok()
        } else if let Ok(span) = btn.find(By::Css("span")).await {
            span.text().await.ok()
        } else {
            let text = btn.text().await.unwrap_or_default();
            let lines: Vec<&str> = text.split('\n').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            lines.iter()
                .find(|line| {
                    let num_chars: String = line.chars().filter(|c: &char| c.is_numeric() || *c == '.').collect();
                    !num_chars.is_empty() && num_chars.parse::<f64>().is_ok()
                })
                .map(|s| s.to_string())
        };

        let raw = amount_text.unwrap_or_else(|| "0".to_string());
        let num_part: String = raw.chars().filter(|c: &char| c.is_numeric() || *c == '.').collect();

        num_part
            .parse()
            .map_err(|_| format!("Cannot parse amount: '{}'", raw))
    }

    // ============================================
    // AVIATOR GAME STATE & AUTOMATION
    // ============================================

    pub async fn get_game_state(&mut self) -> Result<GameState, String> {
        self.enter_aviator_iframe().await?;
        sleep(Duration::from_millis(500)).await;

        let mut state = GameState {
            bet_button_class: String::new(),
            bet_button_text: String::new(),
            cashout_amount: None,
            input_value: None,
            is_waiting: false,
            is_cancel_without_tooltip: false,
            is_cashout_available: false,
            tooltip_text: None,
            raw_html: String::new(),
        };

        let first_panel = "app-bet-control:first-of-type";

        // === FIND MAIN ACTION BUTTON (thorough scan) ===
        let btn = {
            let mut found = None;
            let mut found_via = String::new();

            // Priority 1: State-specific selectors (most reliable)
            let priority_selectors: Vec<String> = vec![
                format!("{} .buttons-block .btn.btn-warning.cashout", first_panel),
                format!("{} .buttons-block .btn-warning", first_panel),
                format!("{} .buttons-block .btn.cashout", first_panel),
                format!("{} .buttons-block [class*='cashout']", first_panel),
                format!("{} .buttons-block .btn.btn-danger", first_panel),
                format!("{} .buttons-block .btn-danger", first_panel),
                format!("{} .buttons-block .btn.btn-success.bet", first_panel),
                format!("{} .buttons-block .btn-success", first_panel),
                format!("{} .buttons-block .btn", first_panel),
                format!("{} .buttons-block button", first_panel),
            ];

            for selector in &priority_selectors {
                if let Ok(b) = self.driver.find(By::Css(selector)).await {
                    let cls = b.attr("class").await.ok().flatten().unwrap_or_default();
                    let txt = b.text().await.unwrap_or_default().to_lowercase();
                    // Validate: must look like an action button
                    if cls.contains("btn-success") || cls.contains("btn-danger")
                        || cls.contains("btn-warning") || cls.contains("cashout")
                        || cls.contains("bet") || txt.contains("bet")
                        || txt.contains("cash") || txt.contains("cancel") {
                        found = Some(b);
                        found_via = selector.clone();
                        break;
                    }
                }
            }

            // Priority 2: Broad scan all buttons in panel, score by relevance
            if found.is_none() {
                let broad = format!("{} button, {} .btn", first_panel, first_panel);
                if let Ok(all_btns) = self.driver.find_all(By::Css(&broad)).await {
                    for b in all_btns {
                        let cls = b.attr("class").await.ok().flatten().unwrap_or_default();
                        let txt = b.text().await.unwrap_or_default().to_lowercase();

                        let score =
                            if cls.contains("btn-success") || cls.contains("btn-danger")
                                || cls.contains("btn-warning") { 3 }
                            else if cls.contains("cashout") || cls.contains("bet") { 2 }
                            else if txt.contains("bet") || txt.contains("cash")
                                || txt.contains("cancel") { 2 }
                            else if cls.contains("btn") { 1 }
                            else { 0 };

                        if score >= 2 {
                            found = Some(b);
                            found_via = format!("broad_scan(score={})", score);
                            break;
                        }
                    }
                }
            }

            found
        };

        if let Some(btn) = btn {
            let cls = btn.attr("class").await.ok().flatten().unwrap_or_default();
            let text = btn.text().await.unwrap_or_default().trim().to_string();

            state.bet_button_class = cls.clone();
            state.bet_button_text = text.clone();

            // === DETECT WAITING STATE (tooltip) ===
            // Try child first, then sibling
            if let Ok(tip) = btn.find(By::Css(".btn-tooltip")).await {
                let tip_text = tip.text().await.unwrap_or_default();
                state.is_waiting = tip_text.to_lowercase().contains("waiting")
                    || tip_text.to_lowercase().contains("next round");
                state.tooltip_text = Some(tip_text.trim().to_string());
            } else {
                let tip_sel = format!("{} .buttons-block .btn-tooltip", first_panel);
                if let Ok(tip) = self.driver.find(By::Css(&tip_sel)).await {
                    let tip_text = tip.text().await.unwrap_or_default();
                    state.is_waiting = tip_text.to_lowercase().contains("waiting")
                        || tip_text.to_lowercase().contains("next round");
                    state.tooltip_text = Some(tip_text.trim().to_string());
                }
            }

            // === DETECT CASHOUT (flexible: class OR text OR amount label) ===
            let is_cashout_by_class = cls.contains("btn-warning")
                || cls.contains("cashout")
                || cls.contains("btn-cashout")
                || cls.contains("warning");

            let is_cashout_by_text = text.to_lowercase().contains("cash")
                || text.to_lowercase().contains("out");

            let has_amount_label = btn.find(By::Css("label.amount")).await.is_ok()
                || btn.find(By::Css(".amount")).await.is_ok()
                || btn.find(By::Css("span.amount")).await.is_ok();

            if is_cashout_by_class || is_cashout_by_text || has_amount_label {
                state.is_cashout_available = true;

                // Extract cashout amount from multiple possible locations
                for selector in ["label.amount span", ".amount span", "span.amount", "span"] {
                    if let Ok(el) = btn.find(By::Css(selector)).await {
                        if let Ok(txt) = el.text().await {
                            let trimmed = txt.trim().to_string();
                            if !trimmed.is_empty() {
                                state.cashout_amount = Some(trimmed);
                                break;
                            }
                        }
                    }
                }

                // Fallback: parse numeric value from button text lines
                if state.cashout_amount.is_none() {
                    for line in text.split('\n').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let nums: String = line.chars().filter(|c| c.is_numeric() || *c == '.').collect();
                        if !nums.is_empty() && nums.parse::<f64>().is_ok() {
                            state.cashout_amount = Some(line.to_string());
                            break;
                        }
                    }
                }
            }

            // === DETECT CANCEL WITHOUT TOOLTIP (round just started) ===
            if cls.contains("btn-danger") && !state.is_waiting && !state.is_cashout_available {
                state.is_cancel_without_tooltip = true;
            }
        }

        // === FIND BET INPUT ===
        let input_el = {
            let mut inp = None;
            for selector in [
                &format!("{} .spinner.big input[inputmode=\"decimal\"]", first_panel),
                &format!("{} .bet-block input[inputmode=\"decimal\"]", first_panel),
                &format!("{} input[inputmode=\"decimal\"]", first_panel),
            ] {
                if let Ok(i) = self.driver.find(By::Css(selector)).await {
                    inp = Some(i);
                    break;
                }
            }
            inp
        };

        if let Some(input) = input_el {
            let mut val = input.prop("value").await.ok().flatten();
            if val.is_none() {
                val = input.attr("value").await.ok().flatten();
            }
            state.input_value = val;
        }

        // === DEBUG: Capture first 15 buttons for diagnostics ===
        let mut debug_btns = Vec::new();
        if let Ok(all_btns) = self.driver.find_all(By::Css("button, .btn")).await {
            for b in all_btns.iter().take(15) {
                let c = b.attr("class").await.ok().flatten().unwrap_or_default();
                let t = b.text().await.unwrap_or_default().trim().to_string();
                debug_btns.push(serde_json::json!({
                "class": c.chars().take(80).collect::<String>(),
                "text": t.chars().take(50).collect::<String>()
            }));
            }
        }
        state.raw_html = serde_json::to_string(&debug_btns).unwrap_or_default();

        self.exit_iframe().await;
        Ok(state)
    }
    pub async fn set_bet_input(&mut self, value: &str) -> Result<bool, String> {
        let first_panel = "app-bet-control:first-of-type";
        let selectors = [
            &format!("{} .spinner.big input[inputmode=\"decimal\"]", first_panel),
            &format!("{} .bet-block input[inputmode=\"decimal\"]", first_panel),
            &format!("{} input[inputmode=\"decimal\"]", first_panel),
            "input[inputmode=\"decimal\"]",
            "input[type=\"number\"]",
            "input[type=\"text\"]",
        ];

        for selector in selectors {
            if let Ok(input) = self.driver.find(By::Css(selector)).await {
                let _ = input.click().await;
                sleep(Duration::from_millis(200)).await;

                let _ = input.send_keys("\u{E009}a").await;
                sleep(Duration::from_millis(150)).await;

                let _ = input.send_keys(value).await;
                sleep(Duration::from_millis(200)).await;

                let _ = input.send_keys("\u{E006}").await;
                sleep(Duration::from_millis(300)).await;

                return Ok(true);
            }
        }
        Err("Bet amount input not found".to_string())
    }

    pub async fn click_bet_button(&mut self) -> Result<String, String> {
        let first_panel = "app-bet-control:first-of-type";

        for selector in [
            &format!("{} .buttons-block .btn.btn-success.bet", first_panel),
            &format!("{} .buttons-block .btn-success", first_panel),
            &format!("{} .buttons-block .btn", first_panel),
        ] {
            if let Ok(btn) = self.driver.find(By::Css(selector)).await {
                let cls = btn.attr("class").await.ok().flatten().unwrap_or_default();
                let text = btn.text().await.unwrap_or_default();
                if cls.contains("btn-success") && text.contains("Bet") {
                    let _ = btn.click().await;
                    sleep(Duration::from_millis(300)).await;
                    return Ok("bet_clicked".to_string());
                }
            }
        }

        Err("Bet button not found".to_string())
    }

    pub async fn click_cashout_button(&mut self) -> Result<String, String> {
        let btn = self.find_cashout_button().await?;
        let amount = self.get_cashout_amount(&btn).await.unwrap_or(0.0);

        let _ = btn.click().await;
        sleep(Duration::from_millis(300)).await;

        Ok(format!("cashout_clicked:{}", amount))
    }

    pub async fn auto_bet(&mut self, amount: &str, odd: &str, multiplier: &str) -> Result<String, String> {
        self.enter_aviator_iframe().await?;

        let inner_result = async {
            let first_panel = "app-bet-control:first-of-type";

            self.set_bet_input(amount).await?;
            sleep(Duration::from_millis(300)).await;

            let cashout_val = {
                let o = odd.parse::<f64>().unwrap_or(2.0);
                let m = multiplier.parse::<f64>().unwrap_or(1.5);
                multiply_rounded(o, m, 2)
            };

            let switcher_selectors = [
                &format!("{} .cash-out-switcher .input-switch", first_panel),
                &format!("{} .cash-out-switcher", first_panel),
                ".cash-out-switcher .input-switch",
                ".cash-out-switcher",
            ];

            for selector in switcher_selectors {
                if let Ok(switcher) = self.driver.find(By::Css(selector)).await {
                    let cls = switcher.attr("class").await.ok().flatten().unwrap_or_default();
                    if cls.contains("off") {
                        let _ = switcher.click().await;
                        sleep(Duration::from_millis(300)).await;
                    }
                    break;
                }
            }

            let cashout_input_selectors = [
                &format!("{} .cashout-spinner input[inputmode=\"decimal\"]", first_panel),
                &format!("{} .cashout-spinner input", first_panel),
                ".cashout-spinner input[inputmode=\"decimal\"]",
                ".cashout-spinner input",
            ];

            for selector in cashout_input_selectors {
                if let Ok(input) = self.driver.find(By::Css(selector)).await {
                    let _ = input.send_keys("\u{E009}a").await;
                    sleep(Duration::from_millis(100)).await;
                    let _ = input.send_keys(&cashout_val.to_string()).await;
                    sleep(Duration::from_millis(100)).await;
                    let _ = input.send_keys("\u{E006}").await;
                    sleep(Duration::from_millis(200)).await;
                    break;
                }
            }
            sleep(Duration::from_millis(300)).await;

            let result = self.click_bet_button().await?;
            let _ = self.event_tx.send(BrowserEvent::BetConfirmed);
            Ok(format!("Auto-bet: amount={} odd={} mult={} cashout={} | {}", amount, odd, multiplier, cashout_val, result))
        }.await;

        self.exit_iframe().await;
        inner_result
    }

    pub async fn auto_cashout(&mut self, target_amount: &str) -> Result<String, String> {
        self.enter_aviator_iframe().await?;

        let inner_result = async {
            let btn = self.find_cashout_button().await?;
            let current_amount = self.get_cashout_amount(&btn).await?;

            let target: f64 = target_amount
                .parse()
                .map_err(|_| "Invalid target".to_string())?;

            if float_eq(current_amount, target, 0.01) || current_amount >= target {
                let _ = btn.click().await;
                sleep(Duration::from_millis(300)).await;
                let _ = self.event_tx.send(BrowserEvent::CashoutConfirmed(current_amount));
                Ok(format!("cashed_out:{}", current_amount))
            } else {
                Ok(format!("waiting:{}|target:{}", current_amount, target))
            }
        }.await;

        self.exit_iframe().await;
        inner_result
    }

    // ============================================
    // EVENT-DRIVEN WATCHERS
    // ============================================

    pub async fn start_payout_watcher(&mut self) {
        let mut last_payouts = Vec::new();
        loop {
            match self.get_payouts().await {
                Ok(payouts) => {
                    if payouts != last_payouts {
                        let _ = self.event_tx.send(BrowserEvent::PayoutChanged(payouts.clone()));
                        last_payouts = payouts;
                    }
                }
                Err(e) => {
                    let _ = self.event_tx.send(BrowserEvent::Error(format!("Payout watcher: {}", e)));
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
    }

    pub async fn start_game_state_watcher(&mut self) {
        let mut last_state: Option<GameState> = None;
        loop {
            match self.get_game_state().await {
                Ok(state) => {
                    let changed = last_state.as_ref().map_or(true, |last| {
                        last.bet_button_class != state.bet_button_class
                            || last.bet_button_text != state.bet_button_text
                            || last.is_cashout_available != state.is_cashout_available
                    });

                    if changed {
                        let _ = self.event_tx.send(BrowserEvent::GameStateChanged(state.clone()));
                        last_state = Some(state);
                    }
                }
                Err(e) => {
                    let _ = self.event_tx.send(BrowserEvent::Error(format!("Game state watcher: {}", e)));
                }
            }
            sleep(Duration::from_millis(500)).await;
        }
    }

    // ============================================
    // IFRAME / PAYOUTS / BALANCE / BETTING
    // ============================================

    pub async fn switch_to_iframe(&mut self, index: usize) -> Result<String, String> {
        let iframes = self.driver.find_all(By::Tag("iframe")).await
            .map_err(|e| e.to_string())?;

        if iframes.is_empty() {
            return Err("No iframes found".to_string());
        }

        if let Some(iframe) = iframes.get(index) {
            iframe.clone().enter_frame().await.map_err(|e| e.to_string())?;
            sleep(Duration::from_secs(2)).await;
            let current_url = self.driver.current_url().await.map_err(|e| e.to_string())?;
            Ok(format!("Switched to iframe {} at {}", index, current_url))
        } else {
            Err(format!("Iframe index {} not found ({} total)", index, iframes.len()))
        }
    }

    pub async fn get_payouts(&mut self) -> Result<Vec<String>, String> {
        self.enter_aviator_iframe().await?;

        let payout_elements = self.driver.find_all(By::ClassName("payout")).await
            .map_err(|e| e.to_string())?;

        let mut payouts = Vec::new();
        for el in payout_elements {
            if let Ok(text) = el.text().await {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() && trimmed.contains('x') {
                    payouts.push(trimmed);
                }
            }
        }

        self.exit_iframe().await;
        Ok(payouts)
    }

    pub async fn get_aviator_balance(&mut self) -> Result<AviatorBalance, String> {
        self.enter_aviator_iframe().await?;

        let selectors: Vec<String> = vec![
            ".balance-amount".to_string(),
            ".balance-currency + span".to_string(),
            "[class*='balance']".to_string(),
            "[class*='amount']".to_string(),
            ".header-right span".to_string(),
            ".header span".to_string(),
        ];

        for selector in &selectors {
            if let Ok(el) = self.driver.find(By::Css(selector)).await {
                if let Ok(text) = el.text().await {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        self.exit_iframe().await;
                        let balance_val = if trimmed.parse::<f64>().is_ok() {
                            Some(trimmed.to_string())
                        } else {
                            let re = regex::Regex::new(r"[\d,]+\.?\d*").unwrap();
                            re.find(trimmed).map(|m: regex::Match<'_>| m.as_str().replace(",", ""))
                        };

                        return Ok(AviatorBalance {
                            success: true,
                            balance: balance_val,
                            balance_text: Some(trimmed.to_string()),
                            currency: Some("KES".to_string()),
                            source: Some(format!("selector: {}", selector)),
                            error: None,
                            raw_text: Some(trimmed.to_string()),
                            pending: Some(false),
                            has_amount_span: Some(true),
                            has_currency_span: Some(false),
                            has_header_right: Some(false),
                            has_header: Some(false),
                            angular_ready: Some(true),
                            body_ready: Some(true),
                        });
                    }
                }
            }
        }

        let xpath = "//*[contains(text(), 'KES')]";
        if let Ok(el) = self.driver.find(By::XPath(xpath)).await {
            if let Ok(text) = el.text().await {
                let trimmed = text.trim();
                self.exit_iframe().await;
                return Ok(AviatorBalance {
                    success: true,
                    balance: Some(trimmed.to_string()),
                    balance_text: Some(trimmed.to_string()),
                    currency: Some("KES".to_string()),
                    source: Some("xpath:kes_pattern".to_string()),
                    error: None,
                    raw_text: Some(trimmed.to_string()),
                    pending: Some(false),
                    has_amount_span: Some(true),
                    has_currency_span: Some(false),
                    has_header_right: Some(false),
                    has_header: Some(false),
                    angular_ready: Some(true),
                    body_ready: Some(true),
                });
            }
        }

        self.exit_iframe().await;

        Ok(AviatorBalance {
            success: false,
            balance: None,
            balance_text: None,
            currency: None,
            source: None,
            error: Some("Balance element not found".to_string()),
            raw_text: None,
            pending: Some(true),
            has_amount_span: Some(false),
            has_currency_span: Some(false),
            has_header_right: Some(false),
            has_header: Some(false),
            angular_ready: Some(true),
            body_ready: Some(true),
        })
    }

    pub async fn place_bet(&mut self, amount: &str, _odd: &str, _multiplier: &str) -> Result<String, String> {
        self.enter_aviator_iframe().await?;

        let amount_selectors: Vec<String> = vec![
            "input[type='text']".to_string(),
            ".bet-amount-input".to_string(),
            "input[placeholder*='amount']".to_string(),
            "input[name*='amount']".to_string(),
        ];

        let mut amount_input = None;
        for selector in &amount_selectors {
            if let Ok(input) = self.driver.find(By::Css(selector)).await {
                amount_input = Some(input);
                break;
            }
        }

        if let Some(input) = amount_input {
            input.clear().await.map_err(|e| e.to_string())?;
            input.send_keys(amount).await.map_err(|e| e.to_string())?;
        } else {
            self.exit_iframe().await;
            return Err("Bet amount input not found".to_string());
        }

        let btn_selectors: Vec<String> = vec![
            "button[type='submit']".to_string(),
            ".place-bet-button".to_string(),
            "[data-testid='bet-button']".to_string(),
        ];

        let mut bet_btn = None;
        for selector in &btn_selectors {
            if let Ok(btn) = self.driver.find(By::Css(selector)).await {
                bet_btn = Some(btn);
                break;
            }
        }

        if let Some(btn) = bet_btn {
            btn.click().await.map_err(|e| e.to_string())?;
        } else {
            if let Ok(btn) = self.driver.find(By::XPath("//button[contains(text(), 'Bet')]")).await {
                btn.click().await.map_err(|e| e.to_string())?;
            } else {
                self.exit_iframe().await;
                return Err("Bet button not found".to_string());
            }
        }

        self.exit_iframe().await;
        Ok(format!("Bet placed: KES {}", amount))
    }

    pub async fn click_by_selector(&mut self, selector: &str) -> Result<String, String> {
        let el = self.driver.find(By::Css(selector)).await
            .map_err(|e| e.to_string())?;
        el.click().await.map_err(|e| e.to_string())?;
        Ok("Clicked".to_string())
    }

    pub async fn execute_js(&mut self, script: &str) -> Result<String, String> {
        let result = self.driver.execute(script, vec![]).await
            .map_err(|e| e.to_string())?;
        Ok(result.json().to_string())
    }

    pub async fn quit(&mut self) -> Result<(), String> {
        self.driver.clone().quit().await.map_err(|e| e.to_string())
    }
}