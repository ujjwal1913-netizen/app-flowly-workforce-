use secrecy::Secret;

#[derive(serde::Deserialize, Clone, Debug)]
pub struct MailerSetting {
  pub smtp_host: String,
  pub smtp_port: u16,
  pub smtp_username: String,
  pub smtp_email: String,
  pub smtp_password: Secret<String>,
  pub smtp_tls_kind: String,
}

#[derive(Clone, Debug)]
pub struct BrevoSetting {
  pub api_key: secrecy::Secret<String>,
  pub from_email: String,
  pub from_name: String,
}
