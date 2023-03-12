use std::time::Duration;

// STRINGS
pub const CHECK_WRONG_CHAN: &str = "Command executed from wrong channel";
pub const CHECK_LONG_NAME: &str = "Username too long";
pub const CHECK_LONG_NAME_DESC: &str = "Mojang usernames are no longer than 16 characters.\nWindows 10, Mobile, and Console Editions cannot join.\nContact <@{}> for assistance.";
pub const EMBED_FOOTER: &str = "Bot by Dunkel#0001";
pub const GET_CONN_POOL_ERR: &str = "Unable to get MySQL connection pool.";
pub const MOJANG_API: &str = "https://api.mojang.com/profiles/minecraft";
pub const STEAM_API: &str = "https://api.steampowered.com/ISteamUser/";
pub const STEAM_COMMUNITY: &str = "steamcommunity.com";
pub const WHITELIST_ADD_FAIL: &str = "MCLink failed";

// EMBED STRINGS
pub const CHECK_NOT_MET: &str = "Condition not met";
pub const CONTACT_1: &str = "Contact ";
pub const CONTACT_2: &str = " for assistance.";
pub const GENERAL_FAIL_TITLE: &str = "General Failure";
pub const GENERAL_FAIL_REQUEST: &str = "We are unable to complete this request.";
pub const GENERAL_NOT_ENABLED_TITLE: &str = "Not enabled";
pub const GERERAL_NOT_ENABLED: &str = "This command is not enabled.";
pub const GENERAL_NOT_LINKED: &str = " is not whitelisted.";
pub const MC_FAIL_LINKED_1: &str = "You have already linked a Minecraft account.";
pub const MC_FAIL_LINKED_2: &str = "You may only have one linked Minecraft account at a time.";
pub const NO_RETRY: &str = "Do not retry.";
pub const PUBLIC_SHAMING_TITLE: &str = "Dunkel pls fix bro blease";
pub const PUBLIC_SHAMING_1: &str = "Tell ";
pub const PUBLIC_SHAMING_2: &str = " to fix this.";
pub const STEAM_FAIL_TITLE: &str = "Steamlink Failure";
pub const STEAM_FAIL_CONTACT: &str = "Unable to contact MOON2 Services.";
pub const STEAM_FAIL_LINKED_1: &str = "You have already linked a Steam account.";
pub const STEAM_FAIL_LINKED_2: &str = "You may only have one linked Steam account at a time.";
pub const STEAM_FAIL_NOT_FOUND: &str = "Your Steam Community Profile was not found.";
pub const STEAM_FAIL_REQUEST: &str = "Unable to contact Steam.";
pub const STEAM_FAIL_URL_1: &str = "Your Steam Community Profile link is not correct.";
pub const STEAM_FAIL_URL_2: &str = "Please check your link and try again.";
pub const STEAM_SUCCESS_TITLE: &str = "Steamlink Success";
pub const STEAM_SUCCESS_1: &str = "Your Steam profile ";
pub const STEAM_SUCCESS_2: &str = " was linked successfully.";
pub const STEAM_UNLINK_TITLE: &str = "Steam Unlink Success";
pub const STEAM_UNLINK_SUCCESS: &str = " was unlinked successfully.";
pub const TRY_AGAIN: &str = "Try again in a few minutes.";
pub const UNEXPECTED_FAIL_TITLE: &str = "Unexpected Error";
pub const UNEXPECTED_FAIL: &str = "An unexpected error occurred: ";

// NUMBERS
pub const BOT_AUTHOR: u64 = 82_982_763_317_166_080;
pub const PIT_ROLE: u64 = 195_419_638_182_576_128;
pub const MAX_NAME_LEN: usize = 16;
pub const MC_CHANNEL_ID: u64 = 708_442_959_586_132_009;
pub const RATELIMIT_INTERVAL: Duration = Duration::from_secs(60 * 10);
pub const RATELIMIT_REQUESTS: u16 = 300;
