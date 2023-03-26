mod get_ability_value;

extern crate google_sheets4 as sheets4;
use std::collections::HashSet;

use rand::Rng;

use serenity::framework::standard::help_commands;
use serenity::framework::standard::Args;
use serenity::framework::standard::CommandGroup;
use serenity::framework::standard::HelpOptions;
use serenity::model::prelude::UserId;
use sheets4::hyper::client::HttpConnector;
use sheets4::hyper_rustls::HttpsConnector;
use sheets4::{hyper, hyper_rustls, oauth2, Sheets};

use serenity::async_trait;
use serenity::framework::standard::macros::{command, group, help, hook};
use serenity::framework::standard::{CommandResult, StandardFramework};
use serenity::model::channel::Message;
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::prelude::*;

use crate::get_ability_value::get_ability_value;

use once_cell::sync::OnceCell;

#[derive(Debug)]
struct Config {
    google_application_credentials: String,
    character_spreadsheet_id: String,
    discord_bot_token: String,
}
impl Config {
    pub fn load() -> Config {
        let expect_var =
            |x: &str| dotenv::var(x).unwrap_or_else(|_| panic!("{} env var must be defined!", x));
        Config {
            google_application_credentials: expect_var("GOOGLE_APPLICATION_CREDENTIALS"),
            character_spreadsheet_id: expect_var("CHARACTER_SPREADSHEET_ID"),
            discord_bot_token: expect_var("DISCORD_BOT_TOKEN"),
        }
    }
}

static CONFIG: OnceCell<Config> = OnceCell::new();
static SHEETS: OnceCell<Sheets<HttpsConnector<HttpConnector>>> = OnceCell::new();

struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(check_character)]
struct General;

#[hook]
async fn unknown_command(ctx: &Context, msg: &Message, unknown_command_name: &str) {
    msg.reply(ctx, format!("Unknown command '{}'", unknown_command_name))
        .await
        .ok();
}
#[command]
#[description = "Roll a value on the character sheet of a given character"]
async fn check_character(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let character_name = match args.single_quoted::<String>() {
        Ok(character_name) => character_name,
        Err(_) => {
            msg.reply(ctx, "Please specify a character name as the first argument")
                .await?;
            return Ok(());
        }
    };

    let spreadsheet_id = &CONFIG.get().unwrap().character_spreadsheet_id;

    let sheets_api = SHEETS.get().unwrap();
    // character_name must match one of the sheets
    let (_, spreadsheet) = sheets_api
        .spreadsheets()
        .get(spreadsheet_id)
        .param("fields", "sheets.properties.title")
        .doit()
        .await
        .expect("Unable to fetch sheet");

    if match &spreadsheet.sheets {
        Some(sheets) => !sheets
            .iter()
            .filter_map(|s| s.properties.as_ref())
            .filter_map(|p| p.title.as_ref())
            .any(|t| t == &character_name),
        None => true,
    } {
        msg.reply(
            ctx,
            format!(
                "ERROR: character name '{}' does not correspond to a valid sheet in spreadsheet!",
                character_name
            ),
        )
        .await?;
        return Ok(());
    }

    let first_ability = match args.single_quoted::<String>() {
        Ok(ability) => ability,
        Err(_) => {
            msg.reply(ctx, "Please specify an ability to roll").await?;
            return Ok(());
        }
    };

    let second_ability = match args.single_quoted::<String>() {
        Ok(delimiter) => {
            let second_ability = args.single_quoted::<String>().ok();
            if delimiter != "+" || second_ability.is_none() {
                msg.reply(ctx, "Please specify a second ability to roll in the format: 'first ability' + 'second ability'")
                .await?;
                return Ok(());
            }
            second_ability
        }
        Err(_) => None,
    };

    // get full ability names and ability values from spreadsheets
    let mut ability_value_pairs: [Option<(String, u8)>; 2] = [None, None];
    for (ability, pair) in [Some(first_ability), second_ability]
        .iter()
        .zip(ability_value_pairs.iter_mut())
    {
        let ability = match ability {
            Some(ability) => ability,
            None => break,
        };
        *pair = match get_ability_value(sheets_api, spreadsheet_id, &character_name, ability).await
        {
            Ok(res) => Some(res),
            Err(err) => {
                msg.reply(
                    ctx,
                    format!("ERROR fetching value for ability {}: {}", &ability, err),
                )
                .await?;
                return Ok(());
            }
        };
    }

    // rolll
    let mut rng: rand::rngs::StdRng = rand::SeedableRng::from_entropy();
    let dist: rand::distributions::Uniform<u8> =
        rand::distributions::Uniform::<u8>::new_inclusive(1, 4);
    let pos_roll = rng.sample(dist);
    let neg_roll = rng.sample(dist);
    let (first_ability, second_ability) = (
        ability_value_pairs[0].as_ref().unwrap(),
        &ability_value_pairs[1],
    );
    let expression = match second_ability {
        Some(second_ability) => format!(
            "**{}** rolls **{}**({}) + **{}**({}) + d4({}) - d4({}) = **{}**",
            character_name,
            first_ability.0,
            first_ability.1,
            second_ability.0,
            second_ability.1,
            pos_roll,
            neg_roll,
            first_ability.1 + second_ability.1 + pos_roll - neg_roll
        ),
        None => format!(
            "**{}** rolls **{}**(2x{}) + d4({}) - d4({}) = **{}**",
            character_name,
            first_ability.0,
            first_ability.1,
            pos_roll,
            neg_roll,
            2 * first_ability.1 + pos_roll - neg_roll
        ),
    };

    msg.reply(ctx, expression).await?;
    Ok(())
}

// The framework provides two built-in help commands for you to use.
// But you can also make your own customized help command that forwards
// to the behaviour of either of them.
#[help]
// Define the maximum Levenshtein-distance between a searched command-name
// and commands. If the distance is lower than or equal the set distance,
// it will be displayed as a suggestion.
// Setting the distance to 0 will disable suggestions.
#[max_levenshtein_distance(3)]
// When you use sub-groups, Serenity will use the `indention_prefix` to indicate
// how deeply an item is indented.
// The default value is "-", it will be changed to "+".
#[indention_prefix = "+"]
// Serenity will automatically analyse and generate a hint/tip explaining the possible
// cases of ~~strikethrough-commands~~, but only if
// `strikethrough_commands_tip_in_{dm, guild}` aren't specified.
// If you pass in a value, it will be displayed instead.
async fn my_help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

#[tokio::main]
async fn main() {
    // Load configuration from env, failing if if it is incomplete
    let config = CONFIG.get_or_init(Config::load);

    // Set up Google Sheets API
    let service_account_key =
        oauth2::read_service_account_key(&config.google_application_credentials)
            .await
            .expect("Unable to read application credentials file");

    let auth = oauth2::ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await
        .expect("Failed to create authenticator");

    SHEETS
        .set(Sheets::new(
            hyper::Client::builder().build(
                hyper_rustls::HttpsConnectorBuilder::new()
                    .with_native_roots()
                    .https_or_http()
                    .enable_http1()
                    .enable_http2()
                    .build(),
            ),
            auth,
        ))
        .ok();

    // Set up serenity bot
    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~")) // set the bot's prefix to "~"
        .unrecognised_command(unknown_command)
        .help(&MY_HELP)
        .group(&GENERAL_GROUP);

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&config.discord_bot_token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}
