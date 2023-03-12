use reqwest::{StatusCode, blocking::Client};
use serenity::framework::standard::{Args, ArgError};
use std::{convert::TryFrom, option};
use url::Url;

use crate::constants::*;
use crate::structs::config::Config;

#[derive(Debug)]
pub enum ParseSteamIDError {
  InvalidURL,
  NotFound,
  RequestFailed(StatusCode),
  SerenityParseError,
  UrlParseError,
}

impl From<StatusCode> for ParseSteamIDError {
  fn from(error: StatusCode) -> Self {
    Self::RequestFailed(error)
  }
}

impl<E> From<ArgError<E>> for ParseSteamIDError {
  fn from(_: ArgError<E>) -> Self {
    Self::SerenityParseError
  }
}

impl From<url::ParseError> for ParseSteamIDError {
  fn from(_: url::ParseError) -> Self {
    Self::UrlParseError
  }
}

impl From<option::NoneError> for ParseSteamIDError {
  fn from(_: option::NoneError) -> Self {
    Self::UrlParseError
  }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSummaryResponse {
  pub response: PlayerSummaries,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSummaries {
  pub players: Vec<PlayerSummary>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveVanityURLResponse {
  pub response: ResolveVanityURL,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveVanityURL {
  pub steamid: Option<String>,
  pub success: u32,
  pub message: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSummary {
  pub steamid: String,
  #[serde(alias = "communityvisibilitystate")]
  pub visibility: i8,
  #[serde(alias = "profilestate")]
  pub profile_state: Option<i8>,
  #[serde(alias = "personaname")]
  pub alias: String,
  #[serde(alias = "lastlogoff")]
  pub last_logoff: Option<u64>,
  #[serde(alias = "profileurl")]
  pub profile_url: String,
  pub avatar: String,
  #[serde(alias = "avatarmedium")]
  pub avatar_med: String,
  #[serde(alias = "avatarfull")]
  pub avatar_full: String,
}

impl PlayerSummary {
  pub fn parse_vanity(steamid: String) -> Result<u64, StatusCode> {
    let key = Config::get_config().steam.api_key;
    let client = Client::new();
    let resp = client.get(&format!("{}ResolveVanityURL/v0001/?key={}&vanityurl={}", STEAM_API, key, steamid)).send();

    match resp {
      Ok(res) => {
        match res.status() {
          StatusCode::OK => {
            let ret = res
              .json::<ResolveVanityURLResponse>()
              .unwrap()
              .response
              .steamid;

            match ret {
              Some(val) => Ok(val.parse::<u64>().unwrap()),
              None => Err(StatusCode::NOT_FOUND),
            }
          },
          _ => Err(res.status()),  
        }
      },
      Err(e) => Err(e.status().unwrap()),
    }
  }
}

impl TryFrom<u64> for PlayerSummary {
  type Error = ParseSteamIDError;

  fn try_from(steamid: u64) -> Result<Self, Self::Error> {
    let key = Config::get_config().steam.api_key;
    let client = Client::new();
    let resp = client.get(&format!("{}GetPlayerSummaries/v0002/?key={}&steamids={}", STEAM_API, key, steamid)).send();

    match resp {
      Ok(res) => {
        match res.status() {
          StatusCode::OK => {
            let mut ret: Vec<PlayerSummary> = res
              .json::<PlayerSummaryResponse>()
              .unwrap()
              .response
              .players;
            
            if ret.is_empty() {
              Err(ParseSteamIDError::NotFound)
            } else {
              Ok(ret.remove(0))
            }
          },
          _ => Err(ParseSteamIDError::RequestFailed(res.status())),
        }
      },
      Err(e) => Err(ParseSteamIDError::RequestFailed(e.status().unwrap())),
    }
  }
}

impl TryFrom<String> for PlayerSummary {
  type Error = ParseSteamIDError;

  fn try_from(steamid: String) -> Result<Self, Self::Error> {
    match steamid.parse::<u64>() {
      Ok(val) => Self::try_from(val),
      Err(_) => {
        match Self::parse_vanity(steamid) {
          Ok(va) => Self::try_from(va),
          Err(_) => Err(ParseSteamIDError::NotFound),
        }
      },
    }
  }
}

impl TryFrom<Url> for PlayerSummary {
  type Error = ParseSteamIDError;

  fn try_from(url: Url) -> Result<Self, Self::Error> {
    // Adds a trailing backslash to url if needed
    let mut raw_url = url.into_string();

    if !raw_url.ends_with('/') {
      raw_url.push('/');
    }

    let url = Url::parse(&raw_url)?;

    // Unwrap to see if /profile and /<numbers here> exist
    let mut path = url.path_segments()?;
    // ident_bit is empty string, profile, or id
    let ident_bit = path.next()?;
    // Either a 64-bit integer or a vanity string
    let profile_bit = path.next()?;
    // Return error if URL isn't properly formatted
    if url.host_str()? != STEAM_COMMUNITY
      || ident_bit == ""
      || profile_bit == ""
      // This makes sure no subsequent path segments after /profile/1234 exist
      || path.next()? != "" {
      return Err(ParseSteamIDError::InvalidURL)
    }

    // Match on ident_bit to determine further parsing
    match ident_bit {
      // This is a traditional 64-bit Steamid
      "profiles" => Self::try_from(String::from(ident_bit)),
      // This is a vanity URL
      "id" => Self::try_from(String::from(profile_bit)),
      // Invalid URL
      _ => Err(ParseSteamIDError::InvalidURL),
    }
  }
}

impl TryFrom<&Args> for PlayerSummary {
  type Error = ParseSteamIDError;

  fn try_from(args: &Args) -> Result<Self, Self::Error> {
    let arg = args.parse::<String>()?;

    // Try parse to u64
    if let Ok(val) = arg.parse::<u64>() {
      return PlayerSummary::try_from(val)
    }

    // Then try parse to URL
    if let Ok(val) = Url::parse(&arg) {
      return PlayerSummary::try_from(val)
    }

    // Otherwise try lookup by String
    PlayerSummary::try_from(arg)
  }
}
