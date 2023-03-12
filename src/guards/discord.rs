use dotenv::dotenv;
use reqwest::blocking::Response;
use rocket::{
  data::{FromDataSimple, Outcome},
  http::Status,
  Data,
  Outcome::*,
  Request,
};
use std::convert::TryFrom;
use std::env;
use std::marker::PhantomData;
use std::io::Read;
use url::Url;

use crate::DISCORD_API_ENDPOINT;
use crate::DISCORD_APP_ID;
use crate::DISCORD_REDIRECT_URI;
use crate::DISCORD_SCOPES;

const LIMIT: u64 = 256;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Bearer {
  pub access_token: String,
  pub token_type: String,
  pub expires_in: u64,
  pub refresh_token: String,
  pub scope: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DiscordToken<T> {
  pub token: String,
  #[serde(skip)]
  phantom: PhantomData<T>,
}

#[derive(Debug)]
pub struct AccessCode;

#[derive(Debug)]
pub struct BearerToken;

#[derive(Debug)]
pub struct RefreshToken;

impl<T> FromDataSimple for DiscordToken<T> {
  type Error = ExchangeError;

  fn from_data(_: &Request, data: Data) -> Outcome<Self, Self::Error> {
    let mut string = String::new();

    if data.open().take(LIMIT).read_to_string(&mut string).is_err() {
      return Failure((
        Status::InternalServerError,
        Self::Error::SerializationError,
      ));
    }

    let exchange_key: Result<DiscordToken<T>, serde_json::Error> = serde_json::from_str(&string);

    match exchange_key {
      Ok(val) => Outcome::Success(val),
      Err(_) => Outcome::Failure((Status::Forbidden, Self::Error::Rejected)),
    }
  }
}

impl TryFrom<DiscordToken<RefreshToken>> for Bearer {
  type Error = ExchangeError;

  fn try_from(token: DiscordToken<RefreshToken>) -> Result<Self, Self::Error> {
    // Step 1: Create the token exchange request
    let req = RefreshRequest::new(token.token);

    // Step 2: Make the request to Discord
    let client: Response = reqwest::blocking::Client::new()
      .post(Url::parse(format!("{}/oauth2/token", DISCORD_API_ENDPOINT).as_str()).unwrap())
      .form(&req)
      .send()?;
    
    // Step 3: Determine if the operation is a success
    if client.status().is_success() {
      let res: Bearer = client.json::<Bearer>()?;

      // Step 4: Ensure required scopes are present
      let req_scopes: Vec<&str> = DISCORD_SCOPES.to_vec();
      let scopes: Vec<&str> = res.scope.split(' ').collect();
      for scope in req_scopes {
        if !scopes.contains(&scope) {
          return Err(Self::Error::MissingScope)
        }
      }
      
      Ok(res)
    } else {
      Err(Self::Error::Rejected)
    }    
  }
}

#[derive(Debug)]
pub enum ExchangeError {
  BadCount,
  Missing,
  Rejected,
  ReqwestError,
  SerializationError,
  MissingScope,
}

impl From<reqwest::Error> for ExchangeError {
  fn from(_: reqwest::Error) -> Self {
    ExchangeError::ReqwestError
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RefreshRequest {
  pub client_id: u64,
  pub client_secret: String,
  pub refresh_token: String,
  pub grant_type: String,
  pub redirect_uri: String,
  pub scope: String,
}

impl RefreshRequest {
  pub fn new(refresh_token: String) -> Self {
    dotenv().ok();
    let client_secret = env::var("DISCORD_SECRET").expect("DISCORD_SECRET must be set in .env");
    let mut scope = String::new();

    for intent in DISCORD_SCOPES.iter() {
      scope.push_str(intent);
      scope.push(' ');
    }

    scope = scope
      .trim_end()
      .to_owned();

    Self {
      client_secret,
      refresh_token: refresh_token.to_owned(),
      grant_type: "refresh_token".to_owned(),
      redirect_uri: DISCORD_REDIRECT_URI.into(),
      client_id: DISCORD_APP_ID,
      scope,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExchangeRequest {
  pub client_id: u64,
  pub client_secret: String,
  pub code: String,
  pub grant_type: String,
  pub redirect_uri: String,
}

impl ExchangeRequest {
  pub fn new(code: String) -> Self {
    dotenv().ok();
    let client_secret = env::var("DISCORD_SECRET").expect("DISCORD_SECRET must be set in .env");

    Self {
      client_secret,
      code: code.to_owned(),
      grant_type: "authorization_code".to_owned(),
      redirect_uri: DISCORD_REDIRECT_URI.into(),
      client_id: DISCORD_APP_ID,
    }
  }
}

impl TryFrom<DiscordToken<AccessCode>> for Bearer {
  type Error = ExchangeError;

  fn try_from(code: DiscordToken<AccessCode>) -> Result<Self, Self::Error> {
    // Step 1: Create the token exchange request
    let req = ExchangeRequest::new(code.token);

    // Step 2: Make the request to Discord
    let client: Response = reqwest::blocking::Client::new()
      .post(Url::parse(format!("{}/oauth2/token", DISCORD_API_ENDPOINT).as_str()).unwrap())
      .form(&req)
      .send()?;
    
    // Step 3: Determine if the operation is a success
    if client.status().is_success() {
      let res: Bearer = client.json::<Bearer>()?;

      // Step 4: Ensure required scopes are present
      let req_scopes: Vec<&str> = DISCORD_SCOPES.to_vec();
      let scopes: Vec<&str> = res.scope.split(' ').collect();
      for scope in req_scopes {
        if !scopes.contains(&scope) {
          return Err(Self::Error::MissingScope)
        }
      }
      
      Ok(res)
    } else {
      Err(Self::Error::Rejected)
    }
  }
}
