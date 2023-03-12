#![allow(clippy::implicit_hasher)]

use diesel::{QueryDsl, RunQueryDsl};
use rand::seq::SliceRandom;
use serenity::{
  framework::standard::{
    help_commands,
    Args, CommandGroup, CommandResult, CommandOptions,
    DispatchError, HelpOptions, Reason,
    macros::{command, check, help, group},
  },
  model::{
    channel::Message,
    id::{ChannelId, UserId},
  },
  prelude::Context,
  utils::{Colour, MessageBuilder},
};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::env::home_dir;
use std::fs::OpenOptions;
use std::io::{BufReader, Seek, SeekFrom};
use std::time::{Duration, SystemTime};

use crate::constants::*;
use crate::structs::{
  Account, BlacklistEntry, DieselFind, EligibleUsers,
  MysqlPoolContainer, Ratelimiter, WhitelistEntry,
  config::Config, mojang::MinecraftUser,
  steam::{ParseSteamIDError, PlayerSummary},
};
use crate::sql::MysqlPooledConnection;
use crate::models::{
  Deleteable, Findable, NewMinecraftUser,
  MinecraftUser as MinecraftUserModel,
  SteamUser,
};

#[group]
#[commands(
  hcpick, hcbroadcast, mclink,
  mcunlink, quotastats, steamlink,
  steamunlink,
)]
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

#[check]
#[name = "NotMCWhitelisted"]
#[check_in_help(false)]
#[display_in_help(true)]
pub async fn not_mc_whitelisted(
  ctx: &Context,
  msg: &Message,
  _args: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  check_sender_not_whitelisted(ctx, msg, Account::Mojang).await
}

#[check]
#[name = "NotSteamWhitelisted"]
#[check_in_help(false)]
#[display_in_help(true)]
pub async fn not_steam_whitelisted(
  ctx: &Context,
  msg: &Message,
  _args: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  check_sender_not_whitelisted(ctx, msg, Account::Steam).await
}

#[check]
#[name = "ArgsMCWhitelisted"]
#[check_in_help(false)]
#[display_in_help(true)]
pub async fn args_mc_whitelisted(
  ctx: &Context,
  msg: &Message,
  args: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  check_arg_whitelisted(ctx, msg, args, Account::Mojang).await
}

#[check]
#[name = "ArgsSteamWhitelisted"]
#[check_in_help(false)]
#[display_in_help(true)]
pub async fn args_steam_whitelisted(
  ctx: &Context,
  msg: &Message,
  args: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  check_arg_whitelisted(ctx, msg, args, Account::Steam).await
}

#[check]
#[name = "UserMention"]
#[check_in_help(false)]
#[display_in_help(true)]
async fn is_usr_mention(
  ctx: &Context,
  msg: &Message,
  args: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  if !args.is_empty() {
    let usr = args.parse::<String>().unwrap();
    let prefix = usr.get(0..=1).unwrap().to_string();
    let postfix = usr.chars().last().unwrap().to_string();

    // Is the argument a valid @ mention
    if prefix == *"<@" && postfix == *">" {
      return Ok(())
    }
  }

  let _ = msg.channel_id.send_message(&ctx, |m| {
    m.embed(|em| {
      em.title("Condition not met");
      em.description("Argument must be a user mention");
      em.color(Colour::new(0x00FF_0000));
      em.footer(|f| f.text(EMBED_FOOTER))
    })
  });

  Err(Reason::Log("Supplied arguments doesn't include a mentioned user".to_string()))
}

#[check]
#[name = "ValidAcctLength"]
#[check_in_help(false)]
#[display_in_help(false)]
pub async fn valid_acct_length(
  ctx: &Context,
  msg: &Message,
  args: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
  let account = args
    .parse::<String>()
    .expect("An account should ALWAYS be supplied as an argument parsable to String");

  if account.len() <= MAX_NAME_LEN {
    return Ok(())
  }

  let _ = msg.channel_id.send_message(&ctx, |m| {
    m.embed(|em| {
      em.title(CHECK_LONG_NAME);
      em.description(format!("Mojang usernames are no longer than 16 characters.\nWindows 10, Mobile, and Console Editions cannot join.\nContact <@{}> for assistance.", BOT_AUTHOR));
      em.color(Colour::new(0x00FF_0000));
      em.footer(|f| f.text(EMBED_FOOTER))
    })
  });

  Err(Reason::Log(CHECK_LONG_NAME.to_string()))
}

#[check]
#[name = "MojangRatelimit"]
#[check_in_help(false)]
#[display_in_help(true)]
pub async fn check_mojang_ratelimit(
  ctx: &Context,
  msg: &Message,
  _: &mut Args,
  _: &CommandOptions,
) -> Result<(), Reason> {
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

        let _ = msg.channel_id.send_message(&ctx, |m| {
          m.embed(|em| {
            em.title("Quota reached!");
            em.description(format!("We're currently experiencing heavy load.\nTry again in about {:#?} seconds.", time_remaining.as_secs()));
            em.color(Colour::new(0x00FF_0000));
            em.footer(|f| f.text(EMBED_FOOTER))
          })
        });

        Err(Reason::Log("Quota reached. Try again later.".to_string()))
      }
    },
    None => {
      let _ = msg.channel_id.send_message(&ctx, |m| {
        m.embed(|em| {
          em.title(WHITELIST_ADD_FAIL);
          em.description(format!("There was a general error. Please try again.\nContact <@{}> for assistance.", BOT_AUTHOR));
          em.color(Colour::new(0x00FF_0000));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      Err(Reason::Log("Could not get Ratelimiter".to_string()))
    },
  }
}

#[check]
#[name = "CommandEnabled"]
#[check_in_help(true)]
#[display_in_help(false)]
pub async fn command_enabled(
  ctx: &Context,
  msg: &Message,
  _: &mut Args,
  opts: &CommandOptions,
) -> Result<(), Reason> {
  let data = ctx.data.read();
  let config = {
    let data = ctx.data.read().await;

    data.get::<Config>().expect("Config should ALWAYS be present on global context.")
  };
  let mut names: Vec<String> = opts.names
    .iter()
    .map(|x| String::from(x.to_owned()))
    .collect();
  let mut enabled = false;

  names.drain_filter(|val| {
    if !enabled {
      match val.as_ref() {
        "steamlink" | "steamunlink" => enabled = config.steam.enabled,
        "mclink" | "mcunlink" | "quotastats" => enabled = config.minecraft.enabled,
        _ => {},
      }
    }

    true
  });

  match enabled {
    true => Ok(()),
    false => {
      let _ = msg.channel_id.send_message(&ctx, |m| {
        let desc = MessageBuilder::new()
          .push(GERERAL_NOT_ENABLED)
          .build();
  
        m.embed(|em| {
          em.title(GENERAL_NOT_ENABLED_TITLE);
          em.description(desc);
          em.color(Colour::new(0x00FF_0000));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      Err(Reason::Log(GERERAL_NOT_ENABLED.to_string()))
    }
  }
}

#[command]
#[owners_only]
#[description = "Whitelists the given Steam account."]
#[checks(CommandEnabled, WhitelistChan, NotSteamWhitelisted)]
#[min_args(1)]
#[max_args(1)]
pub async fn steamlink(
  ctx: &Context,
  msg: &Message,
  args: Args,
) -> CommandResult {
  match PlayerSummary::try_from(&args) {
    Ok(val) => {
      // Add account to database
      let user = SteamUser {
        discord_id: *msg.author.id.as_u64(),
        steam_id: val.steamid.parse::<u64>().unwrap(),
      };
      
      // Wackyass thread safety shenanegans
      match get_conn(ctx, msg).await {
        Ok(conn) => {
          let blocking = user.create(&conn);
          match blocking {
            Ok(_) => {
              msg.author.direct_message(&ctx, |m| {
                let desc = MessageBuilder::new()
                  .push(STEAM_SUCCESS_1)
                  .push_mono(&val.alias)
                  .push(STEAM_SUCCESS_2)
                  .build();

                m.embed(|em| {
                  em.title(STEAM_SUCCESS_TITLE);
                  em.thumbnail(val.avatar_med);
                  em.description(desc);
                  em.color(Colour::new(0x0000_960C));
                  em.footer(|f| f.text(EMBED_FOOTER))
                })
              }).await?;
            },
            Err(_) => {
              msg.channel_id.send_message(&ctx, |m| {
                let desc = MessageBuilder::new()
                  .push_line(STEAM_FAIL_CONTACT)
                  .push(CONTACT_1)
                  .mention(&UserId(BOT_AUTHOR))
                  .push(CONTACT_2)
                  .build();

                m.embed(|e| {
                  e.title(STEAM_FAIL_TITLE);
                  e.description(desc);
                  e.color(Colour::new(0x00FF_0000));
                  e.footer(|f| f.text(EMBED_FOOTER))
                })
              }).await?;
            }
          };  
        },
        Err(_) => return Ok(()),
      };

      Ok(())
    },
    Err(e) => match e {
      ParseSteamIDError::InvalidURL => {
        msg.channel_id.send_message(&ctx, |m| {
          let desc = MessageBuilder::new()
            .push_line(STEAM_FAIL_URL_1)
            .push(STEAM_FAIL_URL_2)
            .build();

          m.embed(|em| {
            em.title(STEAM_FAIL_TITLE);
            em.description(desc);
            em.color(Colour::new(0x00FF_0000));
            em.footer(|f| f.text(EMBED_FOOTER))
          })
        }).await?;

        Ok(())
      },
      ParseSteamIDError::NotFound => {
        msg.channel_id.send_message(&ctx, |m| {
          let desc = MessageBuilder::new()
            .push_line(STEAM_FAIL_NOT_FOUND)
            .push(STEAM_FAIL_URL_2)
            .build();

          m.embed(|em| {
            em.title(STEAM_FAIL_TITLE);
            em.description(desc);
            em.color(Colour::new(0x00FF_0000));
            em.footer(|f| f.text(EMBED_FOOTER))
          })
        }).await?;

        Ok(())
      },
      ParseSteamIDError::RequestFailed(_) => {
        msg.channel_id.send_message(&ctx, |m| {
          let desc = MessageBuilder::new()
            .push_line(STEAM_FAIL_REQUEST)
            .push(TRY_AGAIN)
            .build();

          m.embed(|em| {
            em.title(STEAM_FAIL_TITLE);
            em.description(desc);
            em.color(Colour::new(0x00FF_0000));
            em.footer(|f| f.text(EMBED_FOOTER))
          })
        }).await?;

        Ok(())
      },
      _ => {
        msg.channel_id.send_message(&ctx, |m| {
          let desc = MessageBuilder::new()
            .push_line(GENERAL_FAIL_REQUEST)
            .push(NO_RETRY)
            .build();

          m.embed(|em| {
            em.title(GENERAL_FAIL_TITLE);
            em.description(desc);
            em.color(Colour::new(0x00FF_0000));
            em.footer(|f| f.text(EMBED_FOOTER))
          })
        }).await?;

        Ok(())
      }
    },
  }
}

#[command]
#[owners_only]
#[checks(CommandEnabled)]
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

#[command]
#[only_in(guilds)]
#[checks(WhitelistChan, CommandEnabled, ValidAcctLength, NotMCWhitelisted, MojangRatelimit)]
#[description = "Whitelists the given Mojang account."]
#[min_args(1)]
#[max_args(1)]
#[bucket = "whitelist"]
pub async fn mclink(
  ctx: &Context,
  msg: &Message,
  args: Args,
) -> CommandResult {
  // Retrieve the user's current MC UUID
  let account = args.parse::<String>().unwrap();
  let res = MinecraftUser::get_user(&account);
  let json: Vec<MinecraftUser>;

  match res.await {
    Ok(val) => json = val,
    Err(_) => {
      let _ = msg.channel_id.send_message(&ctx, |m| {
        m.embed(|em| {
          em.title("Error communicating with Mojang");
          em.description(format!("Mojang's servers may be down. Try again later.\nContact <@{}> or <@663197294262222870> for assistance.", BOT_AUTHOR));
          em.color(Colour::new(0x00FF_0000));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      return Ok(());
    },
  };

  // If resulting array is empty, then username is not found
  if json.is_empty() {
    let _ = msg.channel_id.send_message(&ctx, |m| {
      m.embed(|em| {
        em.title("Username not found");
        em.description(format!("We couldn't find your Mojang account. Check your spelling and try again.\nThe new MOON2 Launcher makes whitelisting a breeze! Download it from the Discord Store or GitHub today!\nhttps://discordapp.com/store/skus/604009411928784917/moon2-launcher\nhttps://github.com/MOONMOONOSS/HeliosLauncher/releases\nContact <@{}> or <@663197294262222870> for assistance.", BOT_AUTHOR));
        em.color(Colour::new(0x00FF_0000));
        em.footer(|f| f.text(EMBED_FOOTER))
      })
    });
    return Ok(());
  }

  // Overwrite json removing the Some()
  let json: MinecraftUser = json[0].clone();

  // Add account to database
  let user = NewMinecraftUser {
    discord_id: *msg.author.id.as_u64(),
    minecraft_uuid: String::from(&json.id),
    minecraft_name: String::from(&json.name),
  };
  match get_conn(ctx, msg).await {
    Ok(conn) => {
      msg.author.direct_message(&ctx, |m| {
        m.embed(|e| {
          e.title("Success");
          e.description(format!("Your Minecraft account `{}` has been successfully linked.
Please check <#{}> channel pins for server info and FAQ.
The new MOON2 Launcher brings all our servers into one launcher! Download it from the Discord Store or GitHub today!
https://discordapp.com/store/skus/604009411928784917/moon2-launcher
https://github.com/MOONMOONOSS/HeliosLauncher/releases
**If you leave Mooncord for any reason, you will be removed from the whitelist**", json.name, MC_CHANNEL_ID));
          e.color(Colour::new(0x0000_960C));
          e.footer(|f| f.text(EMBED_FOOTER))
        })
      }).await?;

      return Ok(())
    },
    Err(_) => {
      msg.channel_id.send_message(&ctx, |m| {
        m.embed(|e| {
          e.title(WHITELIST_ADD_FAIL);
          e.description(format!("Please try again later.\nContact <@{}> or <@663197294262222870> for assistance.", BOT_AUTHOR));
          e.color(Colour::new(0x00FF_0000));
          e.footer(|f| f.text(EMBED_FOOTER))
        })
      }).await?;
    }
  }

  Ok(())
}

#[command]
#[description = "Unlinks the given User's Mojang account"]
#[checks(CommandEnabled, UserMention, ArgsMCWhitelisted)]
#[min_args(1)]
#[max_args(1)]
#[owners_only]
pub async fn mcunlink(
  ctx: &Context,
  msg: &Message,
  mut args: Args,
) -> CommandResult {
  match get_conn(ctx, msg).await {
    Ok(conn) => {
      let usr = mention_to_user_id(&mut args);
      if usr.delete(Account::Mojang, &conn).is_ok() {
        msg.channel_id.send_message(&ctx, |m| {
          m.embed(|em| {
            em.title("Minecraft Unlink Success");
            em.description(format!("<@{}> was unlinked successfully.", &usr.as_u64()));
            em.color(Colour::new(0x0000_960C));
            em.footer(|f| f.text(EMBED_FOOTER))
          })
        }).await?;
  
        return Ok(())
      }

      msg.reply(ctx, format!("<@{}> was not unlinked.", usr.as_u64())).await?;
      return Ok(())
    },
    Err(_) => {
      msg.reply(ctx, String::from("SQL connection unavailable.")).await?;
      return Ok(())
    }
  }
}

#[command]
#[description = "Choose a player at random to be whitelisted on the Hardcore server"]
#[owners_only]
pub fn hcpick(
  ctx: &mut Context,
  msg: &Message,
  _args: Args,
) -> CommandResult {
  let mut data = ctx.data.write();
  let eligible_usrs = data
    .get_mut::<EligibleUsers>()
    .unwrap();

  let whitelist = OpenOptions::new()
    .read(true)
    .write(true)
    .open(format!("{}/hc-mc/whitelist.json", home_dir().unwrap().display()));
  
  match whitelist {
    Ok(mut file) => {
      let reader = BufReader::new(&file);
      let mut wl_vec: Vec<WhitelistEntry> = serde_json::from_reader(reader).unwrap();
      let chosen_usr = eligible_usrs.choose(&mut rand::thread_rng()).unwrap();
      let chosen_wl_entry = WhitelistEntry {
        uuid: format_uuid(String::from(&chosen_usr.minecraft_uuid)),
        name: String::from(&chosen_usr.minecraft_name),
      };

      wl_vec.push(chosen_wl_entry);

      file.seek(SeekFrom::Start(0));
      serde_json::to_writer(file, &wl_vec);

      let usr = UserId(chosen_usr.discord_id).to_user(&ctx).unwrap();
      let _ = usr.direct_message(&ctx, |m| {
        m.embed(|e| {
          e.title("Hardcore Minecraft Message");
          e.description(format!("Your Minecraft account `{}` has been chosen to play on the Hardcore server.
Server IP: `51.81.48.39:27061`
Version: 1.15.2", &chosen_usr.minecraft_name));
          e.color(Colour::new(0x0000_960C));
          e.footer(|f| f.text(EMBED_FOOTER))
        })
      });
      let _ = msg.reply(&ctx, &format!("<@{}> (`{}`) was picked!", chosen_usr.discord_id, &chosen_usr.minecraft_name));

      // Empty the queue
      eligible_usrs.clear();
    },
    Err(err) => {
      let _ = msg.reply(&ctx, "Failed! Unable to open `whitelist.json` for writing!");
      println!("{:#?}", err);
    }
  }

  Ok(())
}

#[command]
#[description = "Broadcasts a message to all Hardcore players"]
#[owners_only]
pub fn hcbroadcast(
  ctx: &mut Context,
  msg: &Message,
  _args: Args,
) -> CommandResult {
  use crate::diesel::ExpressionMethods;
  use crate::schema::minecrafters::dsl::*;

  let conn = match get_conn(ctx, msg) {
    Ok(val) => val,
    Err(_) => return Ok(()),
  };
  let whitelist = OpenOptions::new()
    .read(true)
    .open(format!("{}/hc-mc/whitelist.json", home_dir().unwrap().display()));

  let banlist = OpenOptions::new()
    .read(true)
    .open(format!("{}/hc-mc/banned-players.json", home_dir().unwrap().display()));

  let message = msg.content_safe(&ctx).split_at(12).1.to_owned();

  match whitelist {
    Ok(file) => {
      let wl_reader = BufReader::new(&file);
      let wl_vec: Vec<WhitelistEntry> = serde_json::from_reader(wl_reader).unwrap();

      if banlist.is_ok() {
        let banlist = banlist.unwrap();
        let bl_reader = BufReader::new(&banlist);
        let bl_vec: Vec<BlacklistEntry> = serde_json::from_reader(bl_reader).unwrap();
        let mut filtered_players: Vec<WhitelistEntry> = vec![];

        for whitelist_player in wl_vec {
          if !bl_vec.iter().any(|k| k.uuid == whitelist_player.uuid) {
            filtered_players.push(whitelist_player);
          }
        }

        for whitelist_player in filtered_players {
          let res = minecrafters
            .filter(minecraft_name.eq(whitelist_player.name))
            .select(discord_id)
            .first(&conn);

          if res.is_ok() {
            match UserId(res.unwrap()).to_user(&ctx) {
              Ok(user) => {
                let _ = user.direct_message(&ctx, |m| {
                  m.embed(|e| {
                    e.title(format!("Exclusive Hardcore Server Broadcast from {}", msg.author.name));
                    e.description(&message);
                    e.color(Colour::from_rgb(235, 168, 0));
                    e.footer(|f| f.text(EMBED_FOOTER))
                  })
                });
              },
              Err(_) => {},
            }
          }
        }
      } else {
        let _ = msg.reply(&ctx, "Failed! Unable to open `banned-players.json` for reading!");
      }
    },
    Err(err) => {
      let _ = msg.reply(&ctx, "Failed! Unable to open `whitelist.json` for reading!");
      println!("{:#?}", err);
    }
  }

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

#[command]
#[description = "Unlinks the given User's Steam account"]
#[checks(CommandEnabled, UserMention, ArgsSteamWhitelisted)]
#[min_args(1)]
#[max_args(1)]
#[owners_only]
pub fn steamunlink(
  ctx: &mut Context,
  msg: &Message,
  mut args: Args,
) -> CommandResult {
  let usr = mention_to_user_id(&mut args);

  let conn = match get_conn(ctx, msg) {
    Ok(val) => val,
    Err(_) => return Err(CommandError(String::from("SQL connection unavailable."))),
  };

  match usr.delete(Account::Steam, &conn) {
    Ok(_) => {
      let _ = msg.channel_id.send_message(&ctx, |m| {
        let desc = MessageBuilder::new()
          .mention(&usr)
          .push(STEAM_UNLINK_SUCCESS)
          .build();

        m.embed(|em| {
          em.title(STEAM_UNLINK_TITLE);
          em.description(desc);
          em.color(Colour::new(0x0000_960C));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      Ok(())
    },
    Err(_) => Err(CommandError(format!("<@{}> was not unlinked.", usr.as_u64()))),
  }
}

async fn get_conn(
  ctx: &Context,
  msg: &Message,
) -> Result<MysqlPooledConnection, CommandResult> {
  let data = ctx.data.read().await;

  match data.get::<MysqlPoolContainer>() {
    Some(v) => v.get().map_err(|_| Ok(())),
    None => {
      let _ = msg.channel_id.send_message(&ctx, |m| {
        m.embed(|em| {
          em.title(WHITELIST_ADD_FAIL);
          em.description(format!("There was a general error. Please try again.\nContact <@{}> for assistance.", BOT_AUTHOR));
          em.color(Colour::new(0x00FF_0000));
          em.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      // TODO: Should probably bubble up a Error Reason
      Err(Ok(()))
    },
  }
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

  let conn = match get_conn(ctx, msg).await {
    Ok(val) => val,
    Err(_) => return Err(Reason::UserAndLog {
      user: "There was a problem looking up that account.".to_string(),
      log: GET_CONN_POOL_ERR.to_string()
    })
  };

  let res: DieselFind;

  match account_type {
    Account::Mojang => res = DieselFind::from(MinecraftUserModel::find(*usr.as_u64(), &conn)),
    Account::Steam => res = DieselFind::from(SteamUser::find(*usr.as_u64(), &conn)),
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

async fn check_sender_not_whitelisted(
  ctx: &Context,
  msg: &Message,
  account_type: Account,
) -> Result<(), Reason> {
  let author_id = msg.author.id.as_u64();
  let conn = match get_conn(ctx, msg).await {
    Ok(val) => val,
    Err(_) => return Err(Reason::UserAndLog{
      user: "There was a problem whitelisting your account.".to_string(),
      log: GET_CONN_POOL_ERR.to_string(),
    }),
  };

  let desc;
  let title;
  let res: DieselFind;

  match account_type {
    Account::Mojang => {
      res = DieselFind::from(MinecraftUserModel::find(*author_id, &conn));
      desc = MessageBuilder::new()
        .push_line(MC_FAIL_LINKED_1)
        .push_line(MC_FAIL_LINKED_2)
        .push_line("The new MOON2 Launcher makes whitelisting a breeze! Download it from the Discord Store or GitHub today!")
        .push_line("https://discordapp.com/store/skus/604009411928784917/moon2-launcher")
        .push_line("https://github.com/MOONMOONOSS/HeliosLauncher/releases")
        .push(CONTACT_1)
        .mention(&UserId(BOT_AUTHOR))
        .push(" or ")
        .mention(&UserId(663_197_294_262_222_870))
        .push(CONTACT_2)
        .build();
      title = WHITELIST_ADD_FAIL;
    },
    Account::Steam => {
      res = DieselFind::from(SteamUser::find(*author_id, &conn));
      desc = MessageBuilder::new()
        .push_line(STEAM_FAIL_LINKED_1)
        .push_line(STEAM_FAIL_LINKED_2)
        .push(CONTACT_1)
        .mention(&UserId(BOT_AUTHOR))
        .push(CONTACT_2)
        .build();
      title = STEAM_FAIL_TITLE;
    },
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
    }
  };

  match res.0 {
    // User found
    None => {
      // Reply to user
      let _ = msg.channel_id.send_message(&ctx, |m| {
        m.embed(|e| {
          e.title(title);
          e.description(desc);
          e.color(Colour::new(0x00FF_0000));
          e.footer(|f| f.text(EMBED_FOOTER))
        })
      });

      Err(Reason::Log("You've already whitelisted a Steam account!".to_string()))
    },
    Some(e) => {
      use diesel::result::Error;

      match e {
        // If we aren't in the database then we are guaranteed to not be whitelisted
        Error::NotFound => Ok(()),
        _ => {
          let desc = MessageBuilder::new()
            .push(UNEXPECTED_FAIL)
            .push_codeblock(e.to_string(), None)
            .push(CONTACT_1)
            .mention(&UserId(BOT_AUTHOR))
            .push(CONTACT_2)
            .build();

          let _ = msg.channel_id.send_message(&ctx, |m| {
            m.embed(|em| {
              em.title(UNEXPECTED_FAIL_TITLE);
              em.description(desc);
              em.color(Colour::new(0x00FF_0000));
              em.footer(|f| f.text(EMBED_FOOTER))
            })
          });

          Err(Reason::UserAndLog {
            user: format!("An unexpected error occurred: `{}`", e.to_string()),
            log: e.to_string()
          })
        }
      }
    }
  }
}
