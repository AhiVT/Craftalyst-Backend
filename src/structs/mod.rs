pub mod config;
pub mod mojang;
pub mod steam;

use diesel::RunQueryDsl;
use diesel::QueryDsl;

use serenity::{
  prelude::{EventHandler, Context},
  model::{
    channel::Message,
    gateway::Ready,
    guild::{Emoji, Guild, Member},
    id::{ChannelId, GuildId, RoleId},
    user::{CurrentUser, User},
  },
  utils::{Colour, MessageBuilder},
};
use serenity::prelude::*;
use std::time::SystemTime;
use csv::{
  ReaderBuilder as CsvReader,
  WriterBuilder as CsvWriter,
};

use crate::structs::{config::{Config, Discord}};
use crate::constants::*;
use crate::sql::MysqlPool;
use crate::models::{Deleteable, Findable, Suspendable, MinecraftUser};

#[derive(Debug)]
pub enum Account {
  All,
  Mojang,
  Steam,
}

// Wrap our tuple in a type we control
#[derive(Debug)]
pub struct Ratelimiter(
  pub SystemTime,
  pub u16,
);

#[derive(Debug, Serialize, Deserialize)]
pub struct WhitelistEntry {
  pub uuid: String,
  pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlacklistEntry {
  pub uuid: String,
  pub name: String,
  pub created: Option<String>,
  pub source: Option<String>,
  pub expires: Option<String>,
  pub reason: Option<String>,
}

impl TypeMapKey for Ratelimiter {
  type Value = Ratelimiter;
}

pub struct EmojiContainer {
  pub val: Emoji,
}

impl TypeMapKey for EmojiContainer {
  type Value = EmojiContainer;
}

pub struct CurrentUserContainer {
  pub val: CurrentUser,
}

impl TypeMapKey for CurrentUserContainer {
  type Value = CurrentUserContainer;
}

#[derive(Debug)]
pub struct DieselFind(pub Option<diesel::result::Error>);

impl<D> From<Result<D, diesel::result::Error>> for DieselFind {
  fn from(res: Result<D, diesel::result::Error>) -> Self {
    match res {
      Ok(_) => Self(None),
      Err(e) => Self(Some(e)),
    }
  }
}

pub struct Handler;

impl EventHandler for Handler {
  fn ready(&self, ctx: Context, ready: Ready) {
    println!("Bot connected as {}", ready.user.name);

    {
      // Insert bot UserId into context
      let mut data = ctx.data.write();
      let config = data.get::<Config>().unwrap();
      let guild_id = GuildId(config.discord.guild_id);
      let guild = Guild::get(&ctx, guild_id).unwrap();
      let emote_name = "moon2Pinged";

      guild.emojis
        .iter()
        .map(|v| {
          if v.1.name == emote_name {
            data.insert::<EmojiContainer>(EmojiContainer{val: v.1.to_owned()});
          }

          v
        })
        .for_each(drop);

      data.insert::<CurrentUserContainer>(CurrentUserContainer{val: ready.user});
    }
  }

  // Dumb meme easter egg
  fn message(&self, ctx: Context, msg: Message) {
    let mut data = ctx.data.write();
    let my_id = User::from(&data
      .get::<CurrentUserContainer>()
      .unwrap()
      .val
    );

    // Check if the user is posting in #minecraft_whitelisting, #minecraft, or #hardcore_lottery
    if msg.channel_id == ChannelId(305_572_919_730_372_618)
      || msg.channel_id == ChannelId(305_577_946_180_091_926)
      || msg.channel_id == ChannelId(707_283_274_082_549_840) {
      let users = CsvReader::new().has_headers(false).from_path("users_given_keys.csv");
      let key_file = CsvReader::new().has_headers(false).from_path("keys.csv");
      
      match users {
        Ok(mut rdr) => {
          let keyed_iter = rdr.records();
          let mut keyed: Vec<u64> = keyed_iter
            .map(|x|
              x
                .unwrap()
                .get(0)
                .unwrap()
                .to_owned()
                .parse::<u64>()
                .unwrap()
            )
            .collect();
          
          if !keyed.contains(&*msg.author.id.as_u64()) && key_file.is_ok() {
            let mut key_rdr = key_file.unwrap();
            let iter = key_rdr.records();
            let mut keys: Vec<String> = iter
              .map(|x|
                x
                  .unwrap()
                  .get(0)
                  .unwrap()
                  .to_owned()
                )
              .collect();
            
            // Pick first key, then delete it
            let chosen_key = &keys[0].to_owned();
            keys.remove(0);

            // Save keys.csv without the taken key
            let mut key_writer = CsvWriter::new().has_headers(false).from_path("keys.csv").unwrap();
            for key in keys.drain(..) {
              key_writer.write_record(&[key]).unwrap();
            }

            key_writer.flush().unwrap();

            let mut keyed_writer = CsvWriter::new().has_headers(false).from_path("users_given_keys.csv").unwrap();
            for user in keyed.drain(..) {
              keyed_writer.write_record(&[user.to_string()]).expect("Write bulk user record");
            }

            keyed_writer.write_record(&[(*msg.author.id.as_u64()).to_string()]).expect("Write latest user record");
            keyed_writer.flush().expect("Flush given user keys to file");

            let _ = msg.author.direct_message(&ctx, |m| {
              m.content(format!("Here's your very own key for MOON2 Launcher: The Minecraft game launcher that brings all of the MOONMOON Minecraft servers together in one place! {}", chosen_key))
            });
          }
        },
        Err(err) => println!("{:#?}", err),
      }
    }

    // Check if the message was sent in #hardcore_lottery and contains the hotword: me
    if msg.channel_id == ChannelId(707_283_274_082_549_840)
      && &msg.content.to_lowercase() == "me" {
      let conn = data
        .get::<MysqlPoolContainer>()
        .expect("get SQL pool")
        .get()
        .expect("get SQL connection");
      
      match MinecraftUser::find(*msg.author.id.as_u64(), &conn) {
        Ok(val) => {
          {
            let eligible_usrs = data
              .get_mut::<EligibleUsers>()
              .unwrap();

            eligible_usrs.insert(0, val);
            eligible_usrs.truncate(50);
          }

          let _ = msg.author.direct_message(&ctx, |m| {
            m.content("You have been placed under consideration. Should you be picked, you will receive another message from this account.")
          });
        },
        Err(_) => {},
      }
    }

    if msg.mentions.contains(&my_id) {
      let emoji = &data.get::<EmojiContainer>().unwrap().val;

      let content = MessageBuilder::new()
        .emoji(emoji)
        .build();

      let _ = msg.author.direct_message(&ctx, |m| {
        m.content(content)
      });
    }
  }

  fn guild_member_removal(
    &self,
    ctx: Context,
    guild: GuildId,
    user: User,
    _member_data: Option<Member>,
  ) {
    let discord_vals: Discord = Config::get_config().discord;

    if &discord_vals.guild_id == guild.as_u64() {
      println!("{} is leaving Mooncord", user.name);

      let data = ctx.data.read();
      let conn = data
        .get::<MysqlPoolContainer>()
        .expect("get SQL pool")
        .get()
        .expect("get SQL connection");

      let _ = user.id.suspend(Account::Mojang, &conn);
      let _ = user.id.delete(Account::Steam, &conn);
    }
  }

  fn guild_member_addition(
    &self,
    ctx: Context,
    guild: GuildId,
    member: Member,
  ) {
    let discord_vals: Discord = Config::get_config().discord;

    if &discord_vals.guild_id == guild.as_u64() {
      let data = ctx.data.read();
      let conn = data
        .get::<MysqlPoolContainer>()
        .expect("get SQL pool")
        .get()
        .expect("get SQL connection");

      use crate::diesel::ExpressionMethods;
      use crate::schema::minecrafters::dsl::*;

      let usr_id = member.user_id();

      match minecrafters.find(usr_id.as_u64()).first::<MinecraftUser>(&conn) {
        Ok(_) => {
          let user = usr_id.to_user(&ctx).unwrap();
          let _ = user.direct_message(&ctx, |m| {
            m.embed(|e| {
              e.title("Welcome back!");
              e.description(format!("We missed you! You are receiving this message because you whitelisted a Minecraft account.
Your account is now ready to play on Vanilla and Modded servers once more!
**If you leave Mooncord again, you will lose access to our servers.**"));
              e.color(Colour::new(0x0000_960C));
              e.footer(|f| f.text(EMBED_FOOTER))
            })
          });

          let _ = diesel::update(minecrafters.find(usr_id.as_u64()))
            .set(suspended.eq(0))
            .execute(&conn);
        },
        _ => {},
      };
    }
  }

  fn guild_member_update(
    &self,
    ctx: Context,
    old: Option<Member>,
    new: Member,
  ) {
    let data = ctx.data.read();
    let conn = data
      .get::<MysqlPoolContainer>()
      .expect("get SQL pool")
      .get()
      .expect("get SQL connection");

    use crate::diesel::ExpressionMethods;
    use crate::schema::minecrafters::dsl::*;

    if new.roles.contains(&RoleId(PIT_ROLE)) {
      let user = new.user_id().to_user(&ctx).unwrap();

      println!("{} was pitted.", user.name);

      match minecrafters.find(user.id.as_u64()).first::<MinecraftUser>(&conn) {
        Ok(_) => {
          let _ = diesel::update(minecrafters.find(user.id.as_u64()))
            .set(suspended.eq(1))
            .execute(&conn);

          let _ = user.direct_message(&ctx, |m| {
            m.embed(|e| {
              e.title("Notice of Punishment");
              e.description(format!("You have been pitted by a moderator for breaking a rule.
In most cases the moderator who issues the punishment will DM you the reason why and duration.
**This bot does not issue, nor revoke any Discord punishments.**
You are receiving this message because you have a Minecraft account on file.
For the duration of your punishment, you will not be able to join the community servers."));
              e.color(Colour::new(0x0000_960C));
              e.footer(|f| f.text(EMBED_FOOTER))
            })
          });
        },
        Err(_) => {},
      }
    }

    match old {
      Some(val) => {
        if val.roles.contains(&RoleId(PIT_ROLE)) {
          let user = new.user_id().to_user(&ctx).unwrap();

          match minecrafters.find(user.id.as_u64()).first::<MinecraftUser>(&conn) {
            Ok(res) => {
              if res.suspended == 1 {
                let _ = diesel::update(minecrafters.find(user.id.as_u64()))
                  .set(suspended.eq(0))
                  .execute(&conn);
                
                let _ = user.direct_message(&ctx, |m| {
                  m.embed(|e| {
                    e.title("Punishment Followup");
                    e.description(format!("Your punishment has expired. You may now rejoin the community servers."));
                    e.color(Colour::new(0x0000_960C));
                    e.footer(|f| f.text(EMBED_FOOTER))
                  })
                });
              }
            },
            Err(_) => {},
          }  
        }
      },
      None => {},
    }
  }
}

pub struct MysqlPoolContainer;

impl TypeMapKey for MysqlPoolContainer {
  type Value = MysqlPool;
}

pub struct EligibleUsers;

impl TypeMapKey for EligibleUsers {
  type Value = Vec<MinecraftUser>;
}
