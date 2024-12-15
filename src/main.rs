mod db;
mod get_ability_value;

extern crate google_sheets4 as sheets4;

use rand::Rng;

use pyo3::prelude::*;
use pyo3::ffi::c_str;

use poise::serenity_prelude as serenity;

use sheets4::hyper::client::HttpConnector;
use sheets4::hyper_rustls::HttpsConnector;
use sheets4::{hyper, hyper_rustls, oauth2, Sheets};

use serenity::async_trait;

use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::prelude::*;

use crate::db::SheetDB;
use crate::get_ability_value::get_ability_value;

use once_cell::sync::OnceCell;

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type PoiseContext<'a> = poise::Context<'a, Data, Error>;

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

type SheetsAPI = Sheets<HttpsConnector<HttpConnector>>;

static CONFIG: OnceCell<Config> = OnceCell::new();
static SHEETS: OnceCell<SheetsAPI> = OnceCell::new();
static mut SHEET_DB: OnceCell<SheetDB> = OnceCell::new();

struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

/// Claim a character sheet
#[poise::command(prefix_command, slash_command, guild_only)]
async fn claim(
    ctx: PoiseContext<'_>,
    #[description = "Character you want to claim"] character_name: Option<String>,
) -> Result<(), Error> {
    let character_name = match character_name {
        Some(character_name) => character_name,
        None => {
            ctx.say("Please specify a character name as the first argument").await?;
            return Ok(());
        }
    };

    let sheets_api = SHEETS.get().unwrap();
    let spreadsheet_id = &CONFIG.get().unwrap().character_spreadsheet_id;

    match assert_character_name(sheets_api, spreadsheet_id, &character_name).await {
        Ok(()) => {}
        Err(m) => {
            ctx.say(m).await?;
            return Ok(());
        }
    }

    let guild_id = ctx.guild_id().unwrap();
    let sheet_db = unsafe { SHEET_DB.get_mut() }.unwrap();

    match sheet_db.store_sheet(guild_id.into(), ctx.author().id.into(), &character_name) {
        Ok(()) => {
            ctx.say(format!("Claimed sheet {}", character_name))
                .await?
        }
        Err(_) => {
            ctx.say(format!("Failed claiming sheet {}", character_name))
                .await?
        }
    };
    Ok(())
}

/// Check which character you have claimed
async fn my_character_impl(ctx: &PoiseContext<'_>) -> Result<String, String> {
    let guild_id = ctx.guild_id().unwrap();
    let author_id = ctx.author().id;
    let sheet_db = unsafe { SHEET_DB.get_mut() }.unwrap();

    match sheet_db.get_sheet(guild_id.into(), author_id.into()) {
        Ok(name) => name.ok_or("You have not claimed a character yet!".to_owned()),
        Err(err) => Err(format!("Failed fetching your character: {}", err)),
    }
}

#[poise::command(prefix_command, slash_command, guild_only)]
async fn my_character(ctx: PoiseContext<'_>) -> Result<(), Error> {
    match my_character_impl(&ctx).await {
        Ok(name) => {
            ctx.say(format!("Your claimed character is {}", name)).await?
        }
        Err(err) => ctx.say(err).await?,
    };
    Ok(())
}

async fn assert_character_name(
    sheets_api: &SheetsAPI,
    spreadsheet_id: &str,
    character_name: &str,
) -> Result<(), String> {
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
            .any(|t| t == character_name),
        None => true,
    } {
        Err(format!(
            "ERROR: character name '{}' does not correspond to a valid sheet in spreadsheet!",
            character_name
        ))
    } else {
        Ok(())
    }
}



async fn check_impl(
    ctx: &PoiseContext<'_>,
    character_name: &str,
    first_ability: &str,
    second_ability: Option<&str>,
) -> Result<(), Error> {
    let spreadsheet_id = &CONFIG.get().unwrap().character_spreadsheet_id;

    let sheets_api = SHEETS.get().unwrap();

    match assert_character_name(sheets_api, spreadsheet_id, character_name).await {
        Ok(()) => {}
        Err(m) => {
            ctx.say(m).await?;
            return Ok(());
        }
    }

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
        *pair = match get_ability_value(sheets_api, spreadsheet_id, character_name, ability).await {
            Ok(res) => Some(res),
            Err(err) => {
                ctx.say(
                    format!("ERROR fetching value for ability {}: {}", &ability, err),
                )
                .await?;
                return Ok(());
            }
        };
    }

    // roll
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

    ctx.say(expression).await?;
    Ok(())
}


/// Roll a value on the character sheet of a given character
#[poise::command(prefix_command, slash_command, guild_only)]
async fn check_character(
    ctx: PoiseContext<'_>,
    #[description = "Character you want to roll for"] character_name: String,
    #[description = "First ability you want to roll"] first_ability: String,
    #[description = "Second ability you want to roll"] second_ability: Option<String>,
) -> Result<(), Error> {
    check_impl(&ctx, &character_name, &first_ability, second_ability.as_ref().map(|x| x.as_str())).await
}

/// Run a chat completion
#[poise::command(prefix_command, slash_command, guild_only)]
async fn completion(
    ctx: PoiseContext<'_>,
    #[description = "Message you want a completion on"] message: String,
) -> Result<(), Error> {
    let py_app = c_str!(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/python/main.py"
    )));
    let from_python = Python::with_gil(|py| -> PyResult<Py<PyAny>> {
        let sys = py.import("sys")?;
        let path = sys.getattr("path")?;
        path.call_method1("append", (".venv/lib/python3.12/site-packages",))?;  // append my venv path
        let app: Py<PyAny> = PyModule::from_code(py, py_app, c_str!(""), c_str!(""))?
        
            .getattr("run")?
            .into();
        app.call1(py, (message,))
    });
    let res = match from_python {
        Ok(res) => {res.to_string()},
        Err(err) => {
            let err_str = err.to_string();
            println!("Error: {}", err_str);
            ctx.say("Internal Error").await?;
            return Ok(());
        }
    };

    ctx.say(&res).await?;
    Ok(())
}

/// Roll a value for your claimed character
#[poise::command(prefix_command, slash_command, guild_only)]
async fn check(
    ctx: PoiseContext<'_>,
    #[description = "First ability you want to roll"] first_ability: String,
    #[description = "Second ability you want to roll"] second_ability: Option<String>,
) -> Result<(), Error> {
    match my_character_impl(&ctx).await {
        Ok(name) => check_impl(&ctx, &name, &first_ability, second_ability.as_ref().map(|x| x.as_str())).await,
        Err(err) => {
            ctx.say(err).await?;
            Ok(())
        }
    }
}

/// Show this menu
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
    ctx: PoiseContext<'_>,
    #[description = "Specific command to show help about"] command: Option<String>,
) -> Result<(), Error> {
    let config = poise::builtins::HelpConfiguration {
        extra_text_at_bottom: "\
Type ?help command for more info on a command.
You can edit your message to the bot and the bot will edit its response.",
        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    // Prepare python interpreter for python calls
    pyo3::prepare_freethreaded_python();

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

    // Set up DB to store sheet mappings
    let sheet_db = SheetDB::open().expect("Failed to open DB");
    unsafe { SHEET_DB.set(sheet_db) }.ok();

    // Set up serenity bot
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                case_insensitive_commands: true,
                ..Default::default()
            },
            commands: vec![claim(), my_character(), check(), check_character(), completion(), help()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

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
