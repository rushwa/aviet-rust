// ============================================
// src/dom_helpers.rs
// ============================================

pub const DOM_HELPERS_SCRIPT: &str = r#"
(function() {
    if (window.__aviet) {
        console.log('[Aviet] DOM API already injected');
        return;
    }

    window.__aviet = {
        version: '2.0.0-thirtyfour',
        injectionStrategy: 'thirtyfour-evaluate',

        getPageInfo: function() {
            var url = window.location.href;
            var urlLower = url.toLowerCase();
            return {
                url: url,
                hostname: window.location.hostname,
                pathname: window.location.pathname,
                isLoginPage: urlLower.includes('login') || !!document.querySelector('input[name="phone-number"]'),
                isHomePage: url === 'https://www.betika.com/en-ke/' || url === 'https://www.betika.com/en-ke',
                isAviatorPage: urlLower.includes('aviator') || !!document.querySelector('.payouts-block'),
                title: document.title,
                readyState: document.readyState,
                hasPayoutsBlock: !!document.querySelector('.payouts-block'),
                hasCanvas: !!document.querySelector('canvas'),
                hasIframe: !!document.querySelector('iframe')
            };
        },

        getPayouts: function() {
            var payouts = [];
            var elements = document.querySelectorAll('.payout');
            elements.forEach(function(el) {
                var text = el.textContent.trim();
                if (text) payouts.push(text);
            });
            return {
                count: payouts.length,
                payouts: payouts,
                latest: payouts[payouts.length - 1] || null
            };
        },

        getLatestPayout: function() {
            var el = document.querySelector('.payout:last-child');
            return el ? el.textContent.trim() : null;
        },

        navigateToLogin: function() {
            if (this.getPageInfo().isLoginPage) {
                return { success: true, alreadyThere: true };
            }
            var link = document.querySelector('a[href*="/login"]');
            if (link) { link.click(); return { success: true, navigated: true }; }
            window.location.href = 'https://www.betika.com/en-ke/login';
            return { success: true, forceNavigated: true };
        },

        navigateToAviator: function() {
            if (this.getPageInfo().isAviatorPage) {
                return { success: true, alreadyThere: true };
            }
            window.onbeforeunload = null;
            var link = document.querySelector('a[href*="aviator"]');
            if (link) { link.click(); return { success: true, navigated: true }; }
            window.location.replace('https://www.betika.com/en-ke/aviator');
            return { success: true, forceNavigated: true };
        },

        login: function(phone, password) {
            var pageInfo = this.getPageInfo();
            if (!pageInfo.isLoginPage) {
                return { success: false, error: 'Not on login page', needsNavigation: true };
            }

            var phoneInput = document.querySelector('input[name="phone-number"]') ||
                            document.querySelector('input[type="tel"]');
            var passwordInput = document.querySelector('input[type="password"]');

            if (!phoneInput || !passwordInput) {
                return { success: false, error: 'Inputs not found' };
            }

            phoneInput.value = phone;
            phoneInput.dispatchEvent(new Event('input', { bubbles: true }));
            phoneInput.dispatchEvent(new Event('change', { bubbles: true }));

            passwordInput.value = password;
            passwordInput.dispatchEvent(new Event('input', { bubbles: true }));
            passwordInput.dispatchEvent(new Event('change', { bubbles: true }));

            var btn = document.querySelector('button[type="submit"]');
            if (btn) { btn.click(); return { success: true, clicked: true }; }

            return { success: false, error: 'Button not found', filled: true };
        },

        getAuthState: function() {
            var hasLogout = !!document.querySelector('a[href*="logout"]');
            var hasProfile = !!document.querySelector('a[href*="profile"]');
            var hasBalance = Array.from(document.querySelectorAll('span, div')).some(function(el) {
                return /KES\s*[\d,]+/i.test(el.textContent) && el.offsetParent !== null;
            });
            return {
                loggedIn: hasLogout || hasProfile || hasBalance,
                url: window.location.href,
                indicators: { hasLogout, hasProfile, hasBalance }
            };
        },

        getAviatorBalance: function() {
            var amountSpan = document.querySelector('.balance-amount');
            if (amountSpan) {
                var amount = amountSpan.textContent.trim();
                if (/^\d+\.?\d*$/.test(amount)) {
                    return { success: true, balance: amount, currency: 'KES', source: 'angular_balance' };
                }
            }
            var allEls = document.querySelectorAll('span, div, b');
            for (var i = 0; i < allEls.length; i++) {
                var text = allEls[i].textContent;
                var match = text.match(/KES\s*([\d,]+\.?\d*)/i);
                if (match && allEls[i].offsetParent !== null) {
                    return { success: true, balance: match[1].replace(/,/g, ''), currency: 'KES', source: 'page_scan' };
                }
            }
            return { success: false, error: 'Balance not found', pending: true };
        },

        clickAviatorDemo: function() {
            var btn = document.querySelector('button.account__payments__submit.button__secondary.purple');
            if (!btn) {
                var buttons = document.querySelectorAll('button');
                for (var i = 0; i < buttons.length; i++) {
                    var text = buttons[i].textContent.toLowerCase();
                    if (text.includes('demo')) { btn = buttons[i]; break; }
                }
            }
            if (btn) { btn.click(); return { success: true }; }
            return { success: false, error: 'Demo button not found' };
        },

        getSessionInfo: function() {
            return {
                url: window.location.href,
                cookies: document.cookie,
                localStorage: Object.keys(localStorage),
                timestamp: Date.now()
            };
        },

        saveAvietSession: function(data) {
            try {
                localStorage.setItem('aviet_session', JSON.stringify(data));
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
                return { hasSession: !!session, session: session ? JSON.parse(session) : null, age: Date.now() - parseInt(time || '0') };
            } catch(e) {
                return { hasSession: false, error: e.message };
            }
        },

        ping: function() {
            return { pong: true, timestamp: Date.now(), version: this.version };
        }
    };

    console.log('[Aviet] DOM API v2.0.0-thirtyfour injected');
})();
"#;
