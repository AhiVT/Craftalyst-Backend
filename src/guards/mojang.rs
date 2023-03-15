use crate::constants::{RATELIMIT_INTERVAL, RATELIMIT_REQUESTS};

use parking_lot::RwLock;
use std::time::SystemTime;
use rocket::{
  http::Status,
  request::{self, FromRequest, Request as GuardRequest},
  State,
};

#[derive(Debug, Copy, Clone)]
pub struct Ratelimiter{
  pub time: SystemTime,
  pub requests: u16,
}

#[derive(Debug, Copy, Clone)]
pub enum RatelimiterError {
  OverQuota,
  State,
}

impl<'a, 'r> FromRequest<'a, 'r> for Ratelimiter {
  type Error = RatelimiterError;

  fn from_request(request: &'a GuardRequest<'r>) -> request::Outcome<Self, Self::Error> {
    match request.guard::<State<RwLock<Ratelimiter>>>() {
      request::Outcome::Success(limiter) => {
        let mut limiter = limiter.write();
        if (*limiter).time.elapsed().unwrap() > RATELIMIT_INTERVAL {
          (*limiter).time = SystemTime::now();
          (*limiter).requests = 1u16;
        } else if (*limiter).requests < RATELIMIT_REQUESTS {
          (*limiter).requests += 1u16;
        } else {
          return request::Outcome::Failure((Status::TooManyRequests, Self::Error::OverQuota))
        }
      },
      request::Outcome::Failure(_) => return request::Outcome::Failure((Status::InternalServerError, Self::Error::State)),
      _ => return request::Outcome::Failure((Status::InternalServerError, Self::Error::State)),
    };

    match request.guard::<State<RwLock<Ratelimiter>>>()
      .map(|limiter| *(limiter.read())) {
        request::Outcome::Success(send) => request::Outcome::Success(send),
        _ => request::Outcome::Failure((Status::InternalServerError, Self::Error::State)),
      }
  }
}