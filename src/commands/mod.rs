#![allow(clippy::implicit_hasher)]

pub mod mclink;
pub mod mcunlink;

use serenity::{
  framework::standard::{
    help_commands,
    Args, CommandGroup, CommandResult, CommandOptions,
    HelpOptions, Reason,
    macros::{command, check, help, group},
  },
  model::{
    channel::Message,
    id::{ChannelId, UserId}, prelude::interaction::application_command::ApplicationCommandInteraction,
  },
  prelude::Context,
  utils::{Colour, MessageBuilder},
};
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use crate::constants::*;
use crate::structs::{
  Account, DieselFind,
  MysqlPoolContainer, Ratelimiter,
  config::Config,
};
use crate::sql::MysqlPooledConnection;
use crate::models::{
  Findable,
  MinecraftUser as MinecraftUserModel,
};

async fn check_mojang_ratelimit(ctx: &Context) -> Result<(), String> {
  let mut data = ctx.data.write().await;

  match data.get_mut::<Ratelimiter>() {
    Some(mut ratelimiter) => {
      let time = ratelimiter.0;
      let requests = ratelimiter.1;

      if time.elapsed().unwrap() > RATELIMIT_INTERVAL {
        ratelimiter.0 = SystemTime::now();
        // Not zero because this command is also making a request
        ratelimiter.1 = 1u16;

        Ok(())
      } // Executes if under ratelimit quota
      else if requests < RATELIMIT_REQUESTS {
        ratelimiter.1 += 1u16;

        Ok(())
      } else {
        let time_remaining = Duration::from_secs(RATELIMIT_INTERVAL.as_secs() - time.elapsed().unwrap().as_secs());

        Err(format!("We're currently experiencing heavy load.\nTry again in about {:#?} seconds.", time_remaining.as_secs()))
      }
    },
    None => {
      Err(format!("There was a general error. Please try again.\nContact <@{}> for assistance.", BOT_AUTHOR))
    },
  }
}

async fn get_conn(ctx: &Context) -> Result<MysqlPooledConnection, String> {
  let data = ctx.data.read().await;

  match data.get::<MysqlPoolContainer>() {
    Some(v) => v.get().map_err(|err| err.to_string()),
    None => Err(format!("There was a general error. Please try again.\nContact <@{}> for assistance.", BOT_AUTHOR)),
  }
}

async fn check_sender_not_whitelisted(
  ctx: &Context,
  command: &ApplicationCommandInteraction,
  account_type: Account,
) -> Result<(), String> {
  let author_id = match &command.member {
    Some(invoker) => {
      invoker.user.id.as_u64().clone()
    },
    None => return Err("Message sent from DM".to_string()),
  };
  let conn = match get_conn(ctx).await {
    Ok(val) => val,
    Err(_) => return Err(GET_CONN_POOL_ERR.to_string()),
  };

  let res: DieselFind;

  // This may be an issue
  match account_type {
    Account::Mojang => res = DieselFind::from(MinecraftUserModel::find(author_id, &conn)),
    Account::Steam => return Err("Steam linking no longer supported".to_string()),
    _ => return Err("Invalid account type".to_string())
  };

  match res.0 {
    // User found
    None => Err(format!("This account has already been whitelisted by <@{}>", author_id)),
    Some(e) => {
      use diesel::result::Error;

      match e {
        // If we aren't in the database then we are guaranteed to not be whitelisted
        Error::NotFound => Ok(()),
        _ => Err("An unexpected error occurred. You were not whitelisted.".to_string())
      }
    }
  }
}

#[group]
#[commands(quotastats)]
pub struct General;

#[check]
#[name = "WhitelistChan"] // AYAYA CUTE CHANNEL
#[check_in_help(true)]
#[display_in_help(true)]
pub async fn is_whitelist_channel(
  ctx: &Context,
  msg: &Message,
  _: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  let channel_id: ChannelId = {
    let data = ctx.data.read().await;

    let config = data.get::<Config>()
      .expect("Configuration should always be present in Global variables.")
      .clone();

    ChannelId(config.discord.channel_id)
  };

  if channel_id == msg.channel_id {
    Ok(())
  } else {
    Err(Reason::User(CHECK_WRONG_CHAN.to_string()))
  }
}

#[command]
#[owners_only]
pub async fn quotastats(
  ctx: &Context,
  msg: &Message,
  _: Args,
) -> CommandResult {
  match ctx.data.read().await.get::<Ratelimiter>() {
    Some(ratelimiter) => {
      let time = ratelimiter.0;
      let requests = ratelimiter.1;
      let time_remaining = match RATELIMIT_INTERVAL.as_secs().checked_sub(time.elapsed().unwrap().as_secs()) {
        Some(val) => Duration::from_secs(val),
        None => Duration::from_secs(0),
      };
      let quota_text = if time_remaining.as_secs() < 1 {
        String::from("**Quota has refreshed since last execution**")
      } else {
        format!("Quota will refresh in **{:#?}** seconds.", time_remaining.as_secs())
      };

      let _ = msg.channel_id.send_message(&ctx, |m| {
        m.embed(|em| {
          em.title("Mojang Request Quota");
          em.description(format!(
"Quota Limit: **{}**
Quota Remaining: **{}**
{}",
            RATELIMIT_REQUESTS,
            RATELIMIT_REQUESTS - requests,
            quota_text,
          ));
          em.color(Colour::from_rgb(52, 177, 235));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      Ok(())
    },
    None => Ok(()),
  }
}

#[help]
#[max_levenshtein_distance(0)]
#[lacking_permissions = "Strike"]
#[lacking_role = "Strike"]
#[wrong_channel = "Strike"]
pub async fn help(
  ctx: &Context,
  msg: &Message,
  args: Args,
  help_options: &'static HelpOptions,
  groups: &[&'static CommandGroup],
  owners: HashSet<UserId>,
) -> CommandResult {
  help_commands::with_embeds(ctx, msg, args, help_options, groups, owners).await;

  Ok(())
}

fn format_uuid(uuid: String) -> String {
  let first = &uuid[..8];
  let second = &uuid[8..12];
  let third = &uuid[12..16];
  let fourth = &uuid[16..20];
  let fifth = &uuid[20..];

  format!("{}-{}-{}-{}-{}", first, second, third, fourth, fifth)
}

// TODO: Make this a TryFrom
fn mention_to_user_id(args: &mut Args) -> UserId {
  let mut usr = args.parse::<String>().unwrap();

  usr.retain(|c| c.to_string().parse::<i8>().is_ok());

  UserId(usr.parse::<u64>().unwrap())
}

async fn check_arg_whitelisted(
  ctx: &Context,
  msg: &Message,
  args: &mut Args,
  account_type: Account,
) -> Result<(), Reason> {
  // Parse the user string into a UserId
  let usr = mention_to_user_id(args);

  let conn = match get_conn(ctx).await {
    Ok(val) => val,
    Err(_) => return Err(Reason::UserAndLog {
      user: "There was a problem looking up that account.".to_string(),
      log: GET_CONN_POOL_ERR.to_string()
    })
  };

  let res: DieselFind;

  match account_type {
    Account::Mojang => res = DieselFind::from(MinecraftUserModel::find(*usr.as_u64(), &conn)),
    Account::Steam => return Err(Reason::Log("Steam accounts are removed".to_string())),
    _ => {
      let desc = MessageBuilder::new()
        .push(PUBLIC_SHAMING_1)
        .mention(&UserId(BOT_AUTHOR))
        .push(PUBLIC_SHAMING_2)
        .build();

      let _ = msg.channel_id.send_message(&ctx, |m| {
        m.embed(|em| {
          em.title(PUBLIC_SHAMING_TITLE);
          em.description(desc);
          em.color(Colour::new(0x00FF_0000));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      return Err(Reason::Log("Idiot programmer".to_string()))
    },
  };

  match res.0 {
    // User found
    None => Ok(()),
    Some(_) => {
      let _ = msg.channel_id.send_message(&ctx, |m| {
        let desc = MessageBuilder::new()
          .mention(&usr)
          .push(GENERAL_NOT_LINKED)
          .build();

        m.embed(|em| {
          em.title(CHECK_NOT_MET);
          em.description(desc);
          em.color(Colour::new(0x00FF_0000));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      Err(Reason::UserAndLog {
        user: "Not whitelisted".to_string(),
        log: "Not whitelisted".to_string()
      })
    },
  }
}
