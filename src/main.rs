extern crate google_sheets4 as sheets4;
use sheets4::{hyper, hyper_rustls, oauth2, Sheets};

use serenity::async_trait;
use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::{StandardFramework, CommandResult};
use serenity::model::gateway::{GatewayIntents, Ready};

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

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(ping)]
struct General;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    // Load configuration from env, failing if if it is incomplete
    let config = Config::load();


    // Set up Google Sheets API
    let service_account_key =
        oauth2::read_service_account_key(config.google_application_credentials)
            .await
            .expect("Unable to read application credentials file");

    let auth = oauth2::ServiceAccountAuthenticator::builder(service_account_key)
        .build()
        .await
        .expect("Failed to create authenticator");

    let hub = Sheets::new(
        hyper::Client::builder().build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .https_or_http()
                .enable_http1()
                .enable_http2()
                .build(),
        ),
        auth,
    );

    let result = hub
        .spreadsheets()
        .get(&config.character_spreadsheet_id)
        .doit()
        .await
        .expect("Unable to fetch sheet");
    dbg!(result.0);

    // Set up serenity bot
    let framework = StandardFramework::new()
    .configure(|c| c.prefix("~")) // set the bot's prefix to "~"
    .group(&GENERAL_GROUP);

    let intents = GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(config.discord_bot_token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    // start listening for events by starting a single shard
    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }


}
