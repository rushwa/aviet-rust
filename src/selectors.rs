// ============================================
// src/selectors.rs
// ============================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selectors {
    pub name: String,
    pub login_link: String,
    pub login_link_css: String,
    pub login_link_xpath: String,
    pub phone_input: String,
    pub phone_input_css: String,
    pub phone_input_xpath: String,
    pub password_input: String,
    pub password_input_css: String,
    pub password_input_xpath: String,
    pub login_button: String,
    pub login_button_css: String,
    pub login_button_xpath: String,
    pub profile_label: String,
    pub profile_label_css: String,
    pub profile_label_xpath: String,
    pub balance_title: String,
    pub balance_title_css: String,
    pub balance_title_xpath: String,
    pub balance_amount: String,
    pub balance_amount_css: String,
    pub balance_amount_xpath: String,
    pub balance_container_css: String,
    pub aviator_link: String,
    pub aviator_link_css: String,
    pub aviator_link_xpath: String,
    pub dashboard_element: String,
    pub logout_button: String,
    pub error_message: String,
}

impl Selectors {
    pub fn init() -> Self {
        Self {
            name: "Selectors".to_string(),
            login_link: "a.top-session-button[href*='login'], a.button__secondary[href*='login']".to_string(),
            login_link_css: "a.top-session-button:nth-child(1)".to_string(),
            login_link_xpath: "/html/body/div[3]/div[1]/header/div[1]/div[2]/div[1]/a[1]".to_string(),
            phone_input: "input[name='phone-number'], input[placeholder*='0712']".to_string(),
            phone_input_css: "div.input__container:nth-child(2) > input:nth-child(2)".to_string(),
            phone_input_xpath: "/html/body/div[3]/div[1]/div[1]/div[2]/div/div/div[2]/div[1]/input".to_string(),
            password_input: "input[name='password'][type='password']".to_string(),
            password_input_css: "div.input__container:nth-child(1) > input:nth-child(2)".to_string(),
            password_input_xpath: "/html/body/div[3]/div[1]/div[1]/div[2]/div/div/div[2]/div[2]/div/input".to_string(),
            login_button: "button.session__form__button.login, button.button__secondary[type='submit']".to_string(),
            login_button_css: "button.button".to_string(),
            login_button_xpath: "/html/body/div[3]/div[1]/div[1]/div[2]/div/div/div[2]/div[4]/button".to_string(),
            profile_label: "span.nav__item__label".to_string(),
            profile_label_css: "a.topnav__session__links__item:nth-child(3) > span:nth-child(2)".to_string(),
            profile_label_xpath: "/html/body/div[3]/div[1]/header/div[1]/div[2]/div[1]/a[2]/span".to_string(),
            balance_title: "h3.account__info__item__title".to_string(),
            balance_title_css: "div.account__info__item:nth-child(1) > div:nth-child(2) > h3:nth-child(1)".to_string(),
            balance_title_xpath: "/html/body/div[2]/div[1]/div[1]/div[2]/div/div/div[1]/div/div[2]/div/div[1]/div[2]/h3".to_string(),
            balance_amount: "p.account__info__item__value".to_string(),
            balance_amount_css: "div.account__info__item:nth-child(1) > div:nth-child(2) > p:nth-child(2)".to_string(),
            balance_amount_xpath: "/html/body/div[2]/div[1]/div[1]/div[2]/div/div/div[1]/div/div[2]/div/div[1]/div[2]/p".to_string(),
            balance_container_css: "html.no-js.dark body div.app div.desktop-layout div.desktop-layout__content div.desktop-layout__content__center div.account div div.account__info__container.account__section div.account__info.mb-15 div.overlay-menu__strocked-card div.account__info__group div.account__info__item div".to_string(),
            aviator_link: "a[href*='aviator'], span:contains('Aviator')".to_string(),
            aviator_link_css: "a.nav--item:nth-child(5) > span:nth-child(1)".to_string(),
            aviator_link_xpath: "/html/body/div[3]/div[1]/header/div[2]/div/a[5]/span".to_string(),
            dashboard_element: ".user-info, .dashboard, .account-balance".to_string(),
            logout_button: "a[href*='logout'], .logout-btn".to_string(),
            error_message: ".error-message, .alert-danger, .notification--error".to_string(),
        }
    }
}