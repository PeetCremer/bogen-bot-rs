extern crate google_sheets4 as sheets4;
use sheets4::{Sheets, hyper, hyper_rustls, oauth2};
use dotenv;


#[derive(Debug)]
struct Config {
    google_application_credentials: String,
    character_spreadsheet_id: String,
    discord_bot_token: String,
}
impl Config {
    pub fn load() -> Config {
        let expect_var = |x: &str| dotenv::var(x).expect(&format!("{} env var must be defined!", x));
        Config {
            google_application_credentials: expect_var("GOOGLE_APPLICATION_CREDENTIALS"),
            character_spreadsheet_id: expect_var("CHARACTER_SPREADSHEET_ID"),
            discord_bot_token: expect_var("DISCORD_BOT_TOKEN"),
        }
    }
}

#[tokio::main]
async fn main() {
    let config = Config::load();

    let service_account_key = oauth2::read_service_account_key(config.google_application_credentials).await.expect("Unable to read application credentials file");
    let auth = oauth2::ServiceAccountAuthenticator::builder(service_account_key).build().await.expect("Failed to create authenticator");

    let hub = Sheets::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots().https_or_http().enable_http1().enable_http2().build()), auth);

    let result = hub
        .spreadsheets()
        .get(&config.character_spreadsheet_id)
        .doit()
        .await
        .expect("Unable to fetch sheet");
    dbg!(result.0);

}

