#![feature(drain_filter, proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate diesel;
#[macro_use] 
extern crate serde_derive;
// #[macro_use]
// extern crate rocket;
// #[macro_use]
// extern crate rocket_contrib;

pub mod commands;
pub mod constants;
pub mod guards;
pub mod models;
// pub mod routes;
pub mod schema;
pub mod sql;
pub mod structs;

use dotenv::dotenv;
// use parking_lot::RwLock;
// use rocket_contrib::databases::diesel as diesel_rocket;
use serenity::prelude::GatewayIntents;
use serenity::{
  client::Client,
  framework::standard::StandardFramework,
  model::id::UserId,
};
use std::collections::HashSet;
// use std::thread;
use std::time::SystemTime;

// Disable until Craftalyst Client requires this
// use self::routes::*;

use crate::commands::GENERAL_GROUP;
// use crate::guards::mojang::Ratelimiter as APIRatelimiter;
use crate::sql::establish_connection;
use crate::models::MinecraftUser;
use crate::structs::{
  EligibleUsers, Handler, MysqlPoolContainer,
  Ratelimiter, config::Config,
};

static DISCORD_API_ENDPOINT: &str = "https://discordapp.com/api/v6";
static DISCORD_APP_ID: u64 = 1_084_582_096_804_249_622;
// AhiVT Discord
static DISCORD_GUILD_ID: u64 = 951_038_397_726_748_682;
static DISCORD_REDIRECT_URI: &str = "https://localhost:8080/discord";
static DISCORD_SCOPES: [&str; 2] = ["identify", "guilds"];

// #[database("main")]
// pub struct WhitelistDatabase(diesel_rocket::MysqlConnection);

#[tokio::main]
async fn main() {
  dotenv().ok();

  let config = Config::get_config();
  let framework = StandardFramework::new()
    .configure(|c| c.with_whitespace(true).prefix("!"))
    .group(&GENERAL_GROUP)
    .bucket("whitelist", |b| b.time_span(10).limit(1))
    .await;

  let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT;

  let mut client = Client::builder(&config.discord.token, intents)
    .event_handler(Handler)
    .framework(framework)
    .await
    .expect("Error creating client");

  {
    // Bot owners
    // TODO: Make yaml section for list of owner ids
    let mut owners = HashSet::new();
    owners.insert(UserId(82_982_763_317_166_080));  // AhiVT

    let mut data = client.data.write().await;

    // Add connection pool instance to bot
    data.insert::<MysqlPoolContainer>(establish_connection());

    // Make ratelimit counter tuple
    let ratelimit = Ratelimiter(SystemTime::now(), 0u16);
    data.insert::<Ratelimiter>(ratelimit);

    // Used exclusively to pick a random whitelisted user
    let eligible_usrs: Vec<MinecraftUser> = vec![];
    data.insert::<EligibleUsers>(eligible_usrs);

    // Add configuration file to bot
    data.insert::<Config>(config);
  }

  // thread::spawn(move || async {
    // Start listening for events, single shard. Shouldn't need more than one shard
    if let Err(why) = client.start().await {
      eprintln!("An error occurred while running the client: {:?}", why);
    }
  // });

  // println!("Starting API");

  // let ratelimit = RwLock::new(APIRatelimiter {
  //   time: SystemTime::now(),
  //   requests: 0u16,
  // });

  // Disable until Craftalyst Client requires this
  // rocket::ignite()
  //   .attach(WhitelistDatabase::fairing())
  //   .mount("/v1", routes![
  //     login::exchange,
  //     refresh::refresh,
  //     register::register,
  //     status::status,
  //   ])
  //   .manage(ratelimit)
  //   .launch();
}
