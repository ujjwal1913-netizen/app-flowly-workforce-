use actix_web::web::Data;
use actix_web::{web, Scope};
use serde::{Deserialize, Serialize};
use shared_entity::dto::server_info_dto::ServerInfoResponseItem;
use shared_entity::response::{AppResponse, JsonAppResponse};

use crate::state::AppState;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthProvidersPayload {
  pub count: usize,
  pub providers: Vec<String>,
  pub signup_disabled: bool,
  pub mailer_autoconfirm: bool,
  pub custom_providers: Vec<serde_json::Value>,
  pub ldap_providers: Vec<serde_json::Value>,
}

pub fn server_info_scope() -> Scope {
  web::scope("/api/server")
    .service(web::resource("").route(web::get().to(server_info_handler)))
    .service(web::resource("/auth-providers").route(web::get().to(auth_providers_handler)))
    .service(web::resource("/auth-providers/").route(web::get().to(auth_providers_handler)))
}

pub fn auth_providers_scope() -> Scope {
  web::scope("/api/server-info")
    .service(web::resource("/auth-providers").route(web::get().to(auth_providers_handler)))
    .service(web::resource("/auth-providers/").route(web::get().to(auth_providers_handler)))
}

async fn server_info_handler(
  state: Data<AppState>,
) -> actix_web::Result<JsonAppResponse<ServerInfoResponseItem>> {
  Ok(
    AppResponse::Ok()
      .with_data(ServerInfoResponseItem {
        supported_client_features: vec![],
        minimum_supported_client_version: None,
        appflowy_web_url: state.config.appflowy_web_url.clone(),
      })
      .into(),
  )
}

async fn auth_providers_handler(
  state: Data<AppState>,
) -> actix_web::Result<JsonAppResponse<AuthProvidersPayload>> {
  let mut providers = vec!["email".to_string(), "password".to_string()];
  let mut signup_disabled = false;
  let mut mailer_autoconfirm = false;

  if let Ok(settings) = state.gotrue_client.settings().await {
    signup_disabled = settings.disable_signup;
    mailer_autoconfirm = settings.mailer_autoconfirm;
    for p in settings.external.oauth_providers() {
      let p_str = p.to_string();
      if !providers.contains(&p_str) {
        providers.push(p_str);
      }
    }
  }

  let count = providers.len();
  Ok(
    AppResponse::Ok()
      .with_data(AuthProvidersPayload {
        count,
        providers,
        signup_disabled,
        mailer_autoconfirm,
        custom_providers: vec![],
        ldap_providers: vec![],
      })
      .into(),
  )
}

