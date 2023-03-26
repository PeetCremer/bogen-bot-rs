extern crate google_sheets4 as sheets4;

use std::fmt::{self, Display};

use sheets4::client::DefaultDelegate;
use sheets4::{client, hyper, Delegate, Sheets};

use crate::hyper::Uri;
use std::error::Error as StdError;
use tokio::io::{AsyncRead, AsyncWrite};

use std::thread::sleep;

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// Parsing the Csv failed
    CsvError(csv::Error),
    /// Parsing ability from Csv Record failed
    CsvAbilityRecordError(csv::StringRecord),
    /// The http connection failed
    ClientError(client::Error),
    /// The ability was not found
    NoAbilityError(String),
    /// Multiple abilities that could fit were found
    AbilityUniquenessError(String, Vec<String>),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::CsvError(ref err) => err.fmt(f),
            Error::CsvAbilityRecordError(ref rec) => {
                writeln!(f, "Error parsing ability from csv record {:?}", rec)
            }
            Error::ClientError(ref err) => err.fmt(f),
            Error::NoAbilityError(ref ability) => writeln!(f, "No ability {} was found", ability),
            Error::AbilityUniquenessError(ref ability, ref found) => {
                writeln!(f, "Multiple abilities {:?} match {}.", found, ability)
            }
        }
    }
}

use hyper::header::{AUTHORIZATION, USER_AGENT};

pub async fn get_ability_value<S>(
    hub: &Sheets<S>,
    scopes: &[String],
    spreadsheet_id: &str,
    character_name: &str,
    ability: &str,
) -> Result<(String, u8)>
where
    S: tower_service::Service<Uri> + Clone + Send + Sync + 'static,
    S::Response:
        hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    const LIMIT: usize = 3;
    let csv_string =
        get_ability_value_csv(hub, scopes, spreadsheet_id, character_name, ability, LIMIT)
            .await
            .map_err(Error::ClientError)?;

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .quote(b'"')
        .from_reader(csv_string.as_bytes());

    let mut names: Vec<String> = Vec::with_capacity(LIMIT);
    let mut ability_value: Option<(String, u8)> = None;
    for res in reader.records().take(LIMIT) {
        let record = res.map_err(Error::CsvError)?;
        let name = record
            .get(0)
            .ok_or_else(|| Error::CsvAbilityRecordError(record.clone()))?;

        let val = record
            .get(1)
            .ok_or_else(|| Error::CsvAbilityRecordError(record.clone()))?
            .parse::<u8>()
            .map_err(|_| Error::CsvAbilityRecordError(record.clone()))?;

        names.push(name.to_owned());
        ability_value = Some((name.to_owned(), val));
    }
    if names.len() > 1 {
        return Err(Error::AbilityUniquenessError(ability.to_owned(), names));
    }

    ability_value.ok_or_else(|| Error::NoAbilityError(ability.to_owned()))
}

async fn get_ability_value_csv<S>(
    hub: &Sheets<S>,
    scopes: &[String],
    spreadsheet_id: &str,
    character_name: &str,
    ability: &str,
    limit: usize,
) -> client::Result<String>
where
    S: tower_service::Service<Uri> + Clone + Send + Sync + 'static,
    S::Response:
        hyper::client::connect::Connection + AsyncRead + AsyncWrite + Send + Unpin + 'static,
    S::Future: Send + Unpin + 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    let uri: String = format!("https://docs.google.com/spreadsheets/d/{}/gviz/tq?tq=select+A+,+G+where+lower(A)+starts+with+'{}'+limit+{}&sheet={}&tqx=out:csv", spreadsheet_id, ability.to_lowercase(), &limit, character_name);
    let mut dlg = DefaultDelegate;
    loop {
        let token = match hub.auth.token(scopes).await {
            Ok(token) => token.clone(),
            Err(err) => match dlg.token(&err) {
                Some(token) => token,
                None => {
                    dlg.finished(false);
                    return Err(client::Error::MissingToken(err));
                }
            },
        };
        let req_result = {
            let client = &hub.client;
            dlg.pre_request();
            let req_builder = hyper::Request::builder()
                .method(hyper::Method::GET)
                .uri(uri.clone())
                .header(USER_AGENT, "google-api-rust-client/4.0.1".to_string())
                .header(AUTHORIZATION, format!("Bearer {}", token.as_str()));

            let request = req_builder.body(hyper::body::Body::empty());

            client.request(request.unwrap()).await
        };

        match req_result {
            Err(err) => {
                if let client::Retry::After(d) = dlg.http_error(&err) {
                    sleep(d);
                    continue;
                }
                dlg.finished(false);
                return Err(client::Error::HttpError(err));
            }
            Ok(mut res) => {
                let res_body_string = client::get_body_as_string(res.body_mut()).await;
                if !res.status().is_success() {
                    let (parts, _) = res.into_parts();
                    let body = hyper::Body::from(res_body_string.clone());
                    let restored_response = hyper::Response::from_parts(parts, body);

                    let server_response =
                        serde_json::from_str::<serde_json::Value>(&res_body_string).ok();

                    if let client::Retry::After(d) =
                        dlg.http_failure(&restored_response, server_response.clone())
                    {
                        sleep(d);
                        continue;
                    }

                    dlg.finished(false);

                    return match server_response {
                        Some(error_value) => Err(client::Error::BadRequest(error_value)),
                        None => Err(client::Error::Failure(restored_response)),
                    };
                }

                dlg.finished(true);
                return Ok(res_body_string);
            }
        }
    }
}
