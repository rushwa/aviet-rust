pub(crate) const DOM_HELPERS_SCRIPT: &str = r#"
(function() {
    if (window.__aviet) {
        console.log('[Aviet] DOM API already injected');
        return;
    }

    window.__aviet = {
        version: '1.3.0',
        injectionStrategy: 'unknown',

        // ============================================
        // PAGE DETECTION
        // ============================================

        getPageInfo: function() {
            var url = window.location.href;
            var urlLower = url.toLowerCase();

            // Check for aviator in URL or page content
            var hasAviatorContent = !!document.querySelector('.header') ||
                                   !!document.querySelector('.balance-amount') ||
                                   !!document.querySelector('iframe[src*="aviator"]') ||
                                   !!document.querySelector('canvas') ||
                                   urlLower.includes('aviator');

            return {
                url: url,
                hostname: window.location.hostname,
                pathname: window.location.pathname,
                isLoginPage: urlLower.includes('login') || !!document.querySelector('input[name="phone-number"]'),
                isHomePage: url === 'https://www.betika.com/en-ke/' || url === 'https://www.betika.com/en-ke',
                isAviatorPage: hasAviatorContent || urlLower.includes('aviator'),
                title: document.title,
                readyState: document.readyState,
                hasHeader: !!document.querySelector('.header'),
                hasBalanceAmount: !!document.querySelector('.balance-amount'),
                hasCanvas: !!document.querySelector('canvas'),
                hasIframe: !!document.querySelector('iframe')
            };
        },

        // Add to getPageInfo or as separate function
        getAviatorFrame: function() {
            var iframe = document.querySelector('iframe[src*="aviator"]') ||
                        document.querySelector('iframe[src*="spribe"]') ||
                        document.querySelector('iframe[src*="casino"]');

            if (iframe && iframe.contentWindow) {
                try {
                    var frameDoc = iframe.contentWindow.document;
                    var balance = frameDoc.querySelector('.balance-amount');
                    return {
                        hasFrame: true,
                        frameSrc: iframe.src,
                        hasBalanceInFrame: !!balance,
                        balanceText: balance ? balance.textContent : null
                    };
                } catch(e) {
                    return { hasFrame: true, frameSrc: iframe.src, error: 'Cross-origin' };
                }
            }

            return { hasFrame: false };
        },

        // ============================================
        // NAVIGATION
        // ============================================

        navigateToLogin: function() {
            if (this.getPageInfo().isLoginPage) {
                return { success: true, alreadyThere: true, url: window.location.href };
            }

            var loginLink = document.querySelector('a[href*="/login"]') ||
                           document.querySelector('a[href*="login"]');

            if (!loginLink) {
                var allLinks = document.querySelectorAll('a');
                for (var i = 0; i < allLinks.length; i++) {
                    var text = (allLinks[i].textContent || '').toLowerCase().trim();
                    if (text === 'login' || text === 'log in') {
                        loginLink = allLinks[i];
                        break;
                    }
                }
            }

            if (loginLink) {
                loginLink.click();
                return { success: true, navigated: true, url: loginLink.href };
            }

            window.location.href = 'https://www.betika.com/en-ke/login';
            return { success: true, forceNavigated: true };
        },

        navigateToAviator: function() {
            if (this.getPageInfo().isAviatorPage) {
                return { success: true, alreadyThere: true, url: window.location.href };
            }

            // Suppress the "leave page" popup
            window.onbeforeunload = null;
            window.addEventListener('beforeunload', function(e) {
                e.preventDefault();
                e.returnValue = '';
                return '';
            });

            // Remove any existing beforeunload handlers
            var oldHandler = window.onbeforeunload;
            window.onbeforeunload = null;

            // Also try to disable via jQuery if present
            if (typeof jQuery !== 'undefined') {
                jQuery(window).off('beforeunload');
            }

            // Find aviator link
            var aviatorLink = document.querySelector('a[href*="aviator"]');
            if (!aviatorLink) {
                var allLinks = document.querySelectorAll('a');
                for (var i = 0; i < allLinks.length; i++) {
                    var text = (allLinks[i].textContent || '').toLowerCase();
                    if (text.includes('aviator')) {
                        aviatorLink = allLinks[i];
                        break;
                    }
                }
            }

            if (aviatorLink) {
                // Use click instead of href to avoid beforeunload
                aviatorLink.click();
                return { success: true, navigated: true, url: aviatorLink.href, method: 'click' };
            }

            // Force navigation without beforeunload
            var url = 'https://www.betika.com/en-ke/aviator';
            window.location.replace(url);  // replace() doesn't trigger beforeunload
            return { success: true, forceNavigated: true, url: url, method: 'replace' };
        },

        // ============================================
        // COMBINED LOGIN
        // ============================================

        login: function(phone, password) {
            var pageInfo = this.getPageInfo();

            if (!pageInfo.isLoginPage) {
                return {
                    success: false,
                    error: 'Not on login page. Current: ' + pageInfo.url,
                    needsNavigation: true,
                    pageInfo: pageInfo
                };
            }

            var phoneInput = document.querySelector('input[name="phone-number"]') ||
                            document.querySelector('input[type="tel"]') ||
                            document.querySelector('input[placeholder*="07"]') ||
                            document.querySelector('input[placeholder*="phone"]');

            var passwordInput = document.querySelector('input[type="password"]');

            if (!phoneInput || !passwordInput) {
                return {
                    success: false,
                    error: 'Login inputs not found',
                    phoneFound: !!phoneInput,
                    passwordFound: !!passwordInput,
                    pageInfo: pageInfo
                };
            }

            // Fill phone
            phoneInput.focus();
            phoneInput.value = phone;
            ['input', 'change', 'blur', 'keyup'].forEach(function(evt) {
                phoneInput.dispatchEvent(new Event(evt, { bubbles: true }));
            });
            try {
                var setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
                if (setter) setter.call(phoneInput, phone);
                phoneInput.dispatchEvent(new Event('input', { bubbles: true }));
            } catch(e) {}

            // Fill password
            passwordInput.focus();
            passwordInput.value = password;
            ['input', 'change', 'blur', 'keyup'].forEach(function(evt) {
                passwordInput.dispatchEvent(new Event(evt, { bubbles: true }));
            });
            try {
                var setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
                if (setter) setter.call(passwordInput, password);
                passwordInput.dispatchEvent(new Event('input', { bubbles: true }));
            } catch(e) {}

            // Find and click login button
            var loginBtn = null;
            var buttons = document.querySelectorAll('button');
            for (var i = 0; i < buttons.length; i++) {
                var text = (buttons[i].textContent || '').toLowerCase().trim();
                if (text === 'login' || text === 'log in' || text === 'sign in') {
                    loginBtn = buttons[i];
                    break;
                }
            }

            if (!loginBtn) loginBtn = document.querySelector('button[type="submit"]');

            if (!loginBtn) {
                return {
                    success: false,
                    error: 'Login button not found',
                    filled: true,
                    pageInfo: pageInfo
                };
            }

            ['mousedown', 'mouseup', 'click'].forEach(function(evtType) {
                var evt = new MouseEvent(evtType, { bubbles: true, cancelable: true, view: window });
                loginBtn.dispatchEvent(evt);
            });
            loginBtn.click();

            return {
                success: true,
                filled: true,
                clicked: true,
                buttonText: loginBtn.textContent.trim(),
                pageInfo: pageInfo
            };
        },

        // ============================================
        // AUTH STATE
        // ============================================

        getAuthState: function() {
            var url = window.location.href.toLowerCase();
            var phoneInput = document.querySelector('input[name="phone-number"]');

            var hasLogout = !!document.querySelector('a[href*="logout"], .logout-btn, [data-testid="logout"]');
            var hasProfile = !!document.querySelector('a[href*="profile"], .user-profile, [data-testid="profile"]');
            var hasBalance = Array.from(document.querySelectorAll('span, div, p')).some(function(el) {
                return /KES\s*[\d,]+/i.test(el.textContent) && el.offsetParent !== null;
            });
            var hasMyBets = !!document.querySelector('a[href*="my-bets"], a[href*="mybets"]');
            var hasDeposit = !!document.querySelector('a[href*="deposit"]');

            // Check for Angular header balance (Aviator specific)
            var headerBalance = document.querySelector('.header-right .balance');
            var hasAngularBalance = !!headerBalance;

            var isLoggedIn = hasLogout || hasProfile || hasBalance || hasMyBets || hasDeposit || hasAngularBalance;
            var isLoginPage = !!phoneInput || url.includes('login');

            return {
                loggedIn: isLoggedIn,
                onLoginPage: isLoginPage,
                url: window.location.href,
                hasAngularBalance: hasAngularBalance,
                indicators: {
                    hasLogout: hasLogout,
                    hasProfile: hasProfile,
                    hasBalance: hasBalance,
                    hasMyBets: hasMyBets,
                    hasDeposit: hasDeposit
                }
            };
        },

        // ============================================
        // BALANCE - AVIATOR SPECIFIC (from your divs)
        // ============================================

        getAviatorBalance: function() {
            var self = this;

            // Strategy 1: Exact Angular classes from your HTML
            var amountSpan = document.querySelector('.balance-amount');
            var currencySpan = document.querySelector('.balance-currency');

            if (amountSpan && currencySpan) {
                var amount = amountSpan.textContent.trim();
                var currency = currencySpan.textContent.trim();

                // Validate it's a number
                if (/^\d+\.?\d*$/.test(amount)) {
                    return {
                        success: true,
                        balance: amount,
                        balanceText: currency + ' ' + amount,
                        currency: currency,
                        source: 'angular_balance_exact',
                        rawText: amountSpan.textContent
                    };
                }
            }

            // Strategy 2: Just amount span (currency might be hidden)
            if (amountSpan) {
                var amount = amountSpan.textContent.trim();
                if (/^\d+\.?\d*$/.test(amount)) {
                    return {
                        success: true,
                        balance: amount,
                        balanceText: 'KES ' + amount,
                        currency: 'KES',
                        source: 'angular_amount_only',
                        rawText: amountSpan.textContent
                    };
                }
            }

            // Strategy 3: Parent container scan
            var headerRight = document.querySelector('.header-right');
            if (headerRight) {
                var allSpans = headerRight.querySelectorAll('span');
                for (var i = 0; i < allSpans.length; i++) {
                    var text = allSpans[i].textContent.trim();
                    if (/^\d+\.?\d*$/.test(text) && allSpans[i].offsetParent !== null) {
                        var nextSibling = allSpans[i].nextElementSibling;
                        var currency = nextSibling ? nextSibling.textContent.trim() : 'KES';
                        return {
                            success: true,
                            balance: text,
                            balanceText: currency + ' ' + text,
                            currency: currency,
                            source: 'header_right_scan',
                            rawText: text
                        };
                    }
                }
            }

            // Strategy 4: Any KES + number pattern on page top
            var allEls = document.querySelectorAll('span, div, b, strong');
            for (var i = 0; i < allEls.length; i++) {
                var text = allEls[i].textContent;
                var match = text.match(/KES\s*([\d,]+\.?\d*)/i);
                if (match && allEls[i].offsetParent !== null) {
                    var rect = allEls[i].getBoundingClientRect();
                    if (rect.top < 150) {
                        return {
                            success: true,
                            balance: match[1].replace(/,/g, ''),
                            balanceText: text.trim(),
                            currency: 'KES',
                            source: 'page_top_fallback',
                            rawText: text
                        };
                    }
                }
            }

            // Not ready — return debug info for polling
            return {
                success: false,
                error: 'Balance not found',
                pending: true,
                hasAmountSpan: !!amountSpan,
                hasCurrencySpan: !!currencySpan,
                hasHeaderRight: !!headerRight,
                hasHeader: !!document.querySelector('.header'),
                angularReady: !!document.querySelector('[_ngcontent-]'),
                bodyReady: document.readyState === 'complete',
                url: window.location.href
            };
        },

        // ============================================
        // SESSION INFO
        // ============================================

        getSessionInfo: function() {
            var cookies = document.cookie.split(';').map(function(c) {
                return c.trim();
            }).filter(function(c) {
                return c.length > 0;
            });

            var authCookies = cookies.filter(function(c) {
                var lower = c.toLowerCase();
                return lower.includes('session') || lower.includes('auth') || lower.includes('token');
            });

            var lsKeys = Object.keys(localStorage);
            var authKeys = lsKeys.filter(function(k) {
                var lower = k.toLowerCase();
                return lower.includes('token') || lower.includes('auth') || lower.includes('session') || lower.includes('user');
            });

            var authData = {};
            authKeys.forEach(function(k) {
                try {
                    authData[k] = localStorage.getItem(k);
                } catch(e) {}
            });

            return {
                authCookies: authCookies,
                authLocalStorage: authData,
                userAgent: navigator.userAgent,
                onLine: navigator.onLine,
                url: window.location.href,
                timestamp: Date.now()
            };
        },

        // ============================================
        // LOCALSTORAGE SESSION
        // ============================================

        saveAvietSession: function(sessionData) {
            try {
                var data = typeof sessionData === 'string' ? sessionData : JSON.stringify(sessionData);
                localStorage.setItem('aviet_session', data);
                localStorage.setItem('aviet_session_time', Date.now().toString());
                return { success: true };
            } catch(e) {
                return { success: false, error: e.message };
            }
        },

        loadAvietSession: function() {
            try {
                var session = localStorage.getItem('aviet_session');
                var time = localStorage.getItem('aviet_session_time');
                if (!session) return { hasSession: false };

                return {
                    hasSession: true,
                    session: JSON.parse(session),
                    timestamp: parseInt(time || '0'),
                    age: Date.now() - parseInt(time || '0')
                };
            } catch(e) {
                return { hasSession: false, error: e.message };
            }
        },

        clearAvietSession: function() {
            localStorage.removeItem('aviet_session');
            localStorage.removeItem('aviet_session_time');
            return { cleared: true };
        },

        isSessionValid: function(maxAgeMs) {
            maxAgeMs = maxAgeMs || 86400000;
            var loaded = this.loadAvietSession();
            if (!loaded.hasSession) return false;
            return loaded.age < maxAgeMs;
        },

        // ============================================
        // AVIATOR DEMO
        // ============================================

        clickAviatorDemo: function() {
            var btn = document.querySelector('button.account__payments__submit.button__secondary.purple');

            if (!btn) {
                var allButtons = document.querySelectorAll('button');
                for (var i = 0; i < allButtons.length; i++) {
                    var text = (allButtons[i].textContent || '').toLowerCase().replace(/\s+/g, ' ').trim();
                    if (text.includes('click to play demo') || text.includes('play demo') || text.includes('demo')) {
                        btn = allButtons[i];
                        break;
                    }
                }
            }

            if (!btn) {
                return { success: false, error: 'Aviator demo button not found' };
            }

            btn.scrollIntoView({ behavior: 'smooth', block: 'center' });
            ['mousedown', 'mouseup', 'click'].forEach(function(evtType) {
                var evt = new MouseEvent(evtType, { bubbles: true, cancelable: true, view: window });
                btn.dispatchEvent(evt);
            });
            btn.click();

            return { success: true, buttonText: btn.textContent.trim() };
        },

        // ============================================
        // PING
        // ============================================

        ping: function() {
            return {
                pong: true,
                timestamp: Date.now(),
                url: window.location.href,
                version: this.version
            };
        }
    };

    console.log('[Aviet] DOM API v1.3.0 injected');
})();
"#;


// Call any function and get structured JSON back:

// // 1. Check auth state
// webview.evaluate_javascript(
// "JSON.stringify(window.__aviet.getAuthState());",
// None,
// None,
// Some(&cancellable),
// move |result| {
// if let Ok(val) = result {
// let json = val.to_str().to_string();
// // Parse: {"loggedIn": true, "onLoginPage": false, ...}
// println!("Auth: {}", json);
// }
// }
// );
//
// // 2. Fill login form
// let phone = "0712345678";
// let pass = "mypassword";
// let script = format!(
//     "JSON.stringify(window.__aviet.fillLoginForm('{}', '{}'));",
//     escape_js_string(phone),
//     escape_js_string(pass)
// );
// webview.evaluate_javascript(&script, None, None, Some(&cancellable), |_| {});
//
// // 3. Click login
// webview.evaluate_javascript(
// "JSON.stringify(window.__aviet.clickLoginButton());",
// None,
// None,
// Some(&cancellable),
// move |result| {
// if let Ok(val) = result {
// println!("Login click: {}", val.to_str());
// }
// }
// );
//
// // 4. Get balance
// webview.evaluate_javascript(
// "JSON.stringify(window.__aviet.getBalance());",
// None,
// None,
// Some(&cancellable),
// move |result| {
// if let Ok(val) = result {
// let json = val.to_str().to_string();
// // {"balance": "1500.50", "balanceText": "KES 1500.50", "currency": "KES"}
// }
// }
// );
//
// // 5. Get bet slip
// webview.evaluate_javascript(
// "JSON.stringify(window.__aviet.getBetSlip());",
// None,
// None,
// Some(&cancellable),
// move |result| { /* ... */ }
// );
//
// // 6. Set bet amount and place
// let amount = "100";
// let script = format!(
//     "window.__aviet.setBetAmount('{}'); JSON.stringify(window.__aviet.placeBet());",
//     amount
// );
// webview.evaluate_javascript(&script, None, None, Some(&cancellable), |_| {});
//
// // 7. Navigate
// webview.evaluate_javascript(
// "JSON.stringify(window.__aviet.navigateTo('/my-bets'));",
// None,
// None,
// Some(&cancellable),
// |_| {}
// );
//
// // 8. Ping to check if API is alive
// webview.evaluate_javascript(
// "JSON.stringify(window.__aviet.ping());",
// None,
// None,
// Some(&cancellable),
// move |result| {
// if let Ok(val) = result {
// println!("API alive: {}", val.to_str());
// }
// }
// );
//
// use serde::Deserialize;
//
// #[derive(Debug, Deserialize)]
// struct AvietAuthState {
//     loggedIn: bool,
//     onLoginPage: bool,
//     url: String,
//     hasLogout: bool,
//     hasProfile: bool,
//     hasBalance: bool,
//     hasMyBets: bool,
//     phoneInput: bool,
//     passwordInput: bool,
// }
//
// #[derive(Debug, Deserialize)]
// struct AvietBalance {
//     balance: Option<String>,
//     balanceText: Option<String>,
//     currency: String,
// }
//
// #[derive(Debug, Deserialize)]
// struct AvietBetSlip {
//     bets: Vec<<AvietBet>,
//     count: u32,
//     totalOdds: Option<String>,
//     totalStake: Option<String>,
//     potentialWin: Option<String>,
//     hasBets: bool,
// }
//
// #[derive(Debug, Deserialize)]
// struct AvietBet {
//     r#match: Option<String>,
//     odds: Option<String>,
//     currentStake: Option<String>,
// }
//
// #[derive(Debug, Deserialize)]
// struct AvietLoginResult {
//     success: bool,
//     #[serde(default)]
//     error: Option<String>,
//     #[serde(default)]
//     phoneFilled: bool,
//     #[serde(default)]
//     passwordFilled: bool,
// }
//
// fn parse_js_result<T: serde::de::DeserializeOwned>(result: &webkit6::JavascriptResult) -> Option<T> {
//     let json_str = result.to_str().to_string();
//     serde_json::from_str(&json_str).ok()
// }