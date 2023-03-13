pub mod config;
pub mod mojang;

use diesel::RunQueryDsl;
use diesel::QueryDsl;

use serenity::async_trait;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::http::CacheHttp;
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

use crate::commands;
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

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, ctx: Context, ready: Ready) {
    println!("Bot connected as {}", ready.user.name);

    let guild_command = Command::create_global_application_command(&ctx.http, |command| {
      commands::mclink::register(command)
    }).await;

    println!("I created the following global slash command: {:#?}", guild_command);

    {
      // Insert bot UserId into context
      let mut data = ctx
        .data
        .write()
        .await;
      let config = data
        .get::<Config>()
        .unwrap();
      let guild_id = GuildId(config.discord.guild_id);
      let guild = Guild::get(&ctx, guild_id).await.unwrap();
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

  async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
    if let Interaction::ApplicationCommand(command) = interaction {
      println!("Received command interaction: {:#?}", command);

      let owned_command_data = command.data.options.to_vec();
      let content = match command.data.name.as_str() {
        "mclink" => commands::mclink::run(&ctx, &command, &owned_command_data).await,
        _ => "Not implemented".to_string(),
      };

      if let Err(why) = command
        .create_interaction_response(&ctx.http, |response| {
          response
            .kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| message.content(content))
        })
        .await
      {
          println!("Cannot respond to slash command: {}", why);
      }
    }
  }

  // Dumb meme easter egg
  async fn message(&self, ctx: Context, msg: Message) {
    println!("Message sent");
    let mut data = ctx.data.write().await;
    let my_id = User::from(&data
      .get::<CurrentUserContainer>()
      .unwrap()
      .val
    );

    if msg.mentions.contains(&my_id) {
      let emoji = &data.get::<EmojiContainer>().unwrap().val;

      let content = MessageBuilder::new()
        .emoji(emoji)
        .build();

      msg.author.direct_message(&ctx, |m| {
        m.content(content)
      }).await;
    }
  }

  async fn guild_member_removal(
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
        .await
        .get::<MysqlPoolContainer>()
        .expect("get SQL pool")
        .get()
        .expect("get SQL connection");

      let _ = user.id.suspend(Account::Mojang, &conn);
    }
  }

  async fn guild_member_addition(
    &self,
    ctx: Context,
    member: Member,
  ) {
    let discord_vals: Discord = Config::get_config().discord;

    if &discord_vals.guild_id == member.guild_id.as_u64() {
      let data = ctx.data.read();
      let conn = data
        .await
        .get::<MysqlPoolContainer>()
        .expect("get SQL pool")
        .get()
        .expect("get SQL connection");

      use crate::diesel::ExpressionMethods;
      use crate::schema::minecrafters::dsl::*;

      let usr_id = member.user.id;
      let search = minecrafters.find(usr_id.as_u64()).first::<MinecraftUser>(&conn);

      match search {
        Ok(_) => {
          let user = usr_id
            .to_user(&ctx)
            .await
            .unwrap();
          user.direct_message(&ctx, |m| {
            m.embed(|e| {
              e.title("Welcome back!");
              e.description(format!("We missed you! You are receiving this message because you whitelisted a Minecraft account.
Your account is now ready to play on Vanilla and Modded servers once more!
**If you leave Mooncord again, you will lose access to our servers.**"));
              e.color(Colour::new(0x0000_960C));
              e.footer(|f| f.text(EMBED_FOOTER))
            })
          }).await;

          diesel::update(minecrafters.find(usr_id.as_u64()))
            .set(suspended.eq(0))
            .execute(&conn);
        },
        _ => {},
      };
    }
  }

  async fn guild_member_update(
    &self,
    ctx: Context,
    old: Option<Member>,
    new: Member,
  ) {
    let data = ctx.data.read().await;
    let conn = data
      .get::<MysqlPoolContainer>()
      .expect("get SQL pool")
      .get()
      .expect("get SQL connection");

    use crate::diesel::ExpressionMethods;
    use crate::schema::minecrafters::dsl::*;

    if new.roles.contains(&RoleId(PIT_ROLE)) {
      let user = &new.user;

      println!("{} was pitted.", user.name);

      let search = minecrafters.find(user.id.as_u64()).first::<MinecraftUser>(&conn);

      match search {
        Ok(_) => {
          let _ = diesel::update(minecrafters.find(user.id.as_u64()))
            .set(suspended.eq(1))
            .execute(&conn);

          user.direct_message(&ctx, |m| {
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
          }).await;
        },
        Err(_) => {},
      }
    }

    match old {
      Some(val) => {
        if val.roles.contains(&RoleId(PIT_ROLE)) {
          let user = &new.user;

          let search = minecrafters.find(user.id.as_u64()).first::<MinecraftUser>(&conn);
          match search {
            Ok(res) => {
              if res.suspended == 1 {
                let _ = diesel::update(minecrafters.find(user.id.as_u64()))
                  .set(suspended.eq(0))
                  .execute(&conn);
                
                user.direct_message(&ctx, |m| {
                  m.embed(|e| {
                    e.title("Punishment Followup");
                    e.description(format!("Your punishment has expired. You may now rejoin the community servers."));
                    e.color(Colour::new(0x0000_960C));
                    e.footer(|f| f.text(EMBED_FOOTER))
                  })
                }).await;
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
