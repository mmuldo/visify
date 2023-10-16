use std::{
    result,
    io,
    sync::{
        Arc,
        Mutex
    },
    collections::HashMap, process::exit,
};
use inquire::InquireError;
use url::Url;
use rspotify::{
    prelude::*,
    scopes,
    AuthCodePkceSpotify,
    Credentials,
    OAuth,
    ClientError,
};
use rocket;
use crate::config::{Config, app_config_dir};
use arboard::Clipboard;


const CLIENT_ID: &str = "fa974cd060ed42888385234c45c531bb";
const TOKEN_CACHE_FILE: &str = ".spotify_token_cache.json";

const SCOPES: [&str; 5] = [
    "user-library-read",
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-playback-position",
    "user-read-private",
];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("inquire error: {0}")]
    Inquire(#[from] InquireError),
    #[error("Error from spotify client: {0}")]
    Client(#[from] ClientError),
    #[error("Failed to parse url: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Url missing required param: {0}")]
    UrlMissingParam(String),
    #[error("Invalid menu result: {0}")]
    MenuResult(String),
}

pub type Result<T> = result::Result<T, Error>;

#[derive(Default)]
struct TokenRetriever {
    code: String
}

#[rocket::get("/callback?<code>")]
fn callback(
    code: String,
    token_retriever: &rocket::State<Arc<Mutex<TokenRetriever>>>,
    shutdown: rocket::Shutdown
) -> String {
    shutdown.notify();
    
    let mut token_retriever = token_retriever.lock().unwrap();
    token_retriever.code = code;

    "success!".to_string()
}

async fn redirect_uri_web_server() -> result::Result<String, rocket::Error> {
    let token_retriever = Arc::new(Mutex::new(TokenRetriever::default()));
    let rocket_config = rocket::Config {
        port: redirect_uri_port(),
        ..Default::default()
    };

    let _ = rocket::custom(&rocket_config)
        .manage(Arc::clone(&token_retriever))
        .mount("/", rocket::routes![callback])
        .launch()
        .await?;

    let code = token_retriever.lock().unwrap().code.clone();

    Ok(code)
}

async fn get_code(url: &str) -> Result<String> {
    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_text(url).unwrap();

    match webbrowser::open(url) {
        Ok(_) => {
            println!("Opened login page in your browser (login URL also copied to clipboard).");
        }
        Err(error) => eprintln!("Error when trying to open URL in your browser: {error}.
        Please navigate to login page manually (login URL already copied to cpliboard): {url}")
    }

    let maybe_code = redirect_uri_web_server().await;

    match maybe_code {
        Ok(code) => Ok(code),
        Err(error) => {
            eprintln!("Failed to automatically refresh token: {error}");

            let try_callback_url = inquire::Text::new("Please enter redirect URL manually:").prompt();
            match try_callback_url {
                Ok(callback_url) => {
                    let url = Url::parse(&callback_url)?;

                    let params = url.query_pairs().collect::<HashMap<_, _>>();
                    match params.get("code") {
                        Some(code) => Ok(code.to_string()),
                        None => Err(Error::UrlMissingParam("code".to_string()))
                    }
                },
                Err(error) => Err(Error::Inquire(error))
            }
        }
    }
}

async fn get_token(client: &mut AuthCodePkceSpotify, auth_url: &str) -> Result<()> {
    match client.read_token_cache(true).await {
        Ok(Some(new_token)) => {
            let expired = new_token.is_expired();

            // Load token into client regardless of whether it's expired or
            // not, since it will be refreshed later anyway.
            *client.get_token().lock().await.unwrap() = Some(new_token);

            if expired {
                // Ensure that we actually got a token from the refetch
                match client.refetch_token().await? {
                    Some(refreshed_token) => {
                        *client.get_token().lock().await.unwrap() = Some(refreshed_token)
                    }
                    // If not, prompt the user for it
                    None => {
                        let code = get_code(auth_url).await?;
                        client.request_token(&code).await?;
                    }
                }
            }
        }
        // Otherwise following the usual procedure to get the token.
        _ => {
            let code = get_code(auth_url).await?;
            client.request_token(&code).await?;
        }
    }

    Ok(client.write_token_cache().await?)
}

fn redirect_uri_port() -> u16 {
    match Config::load() {
        Ok(config) => config.redirect_uri_port.unwrap(),
        Err(error) => {
            eprintln!("Failed to load redirect uri port from config: {error}.");
            exit(1)
        }
    }
}

fn redirect_uri() -> String {
    format!("http://localhost:{}/callback", redirect_uri_port())
}

pub async fn auth() -> Result<AuthCodePkceSpotify>{
    let creds = Credentials::new_pkce(CLIENT_ID);

    let oauth = OAuth {
        redirect_uri: redirect_uri(),
        scopes: scopes!(&SCOPES.join(" ")),
        ..Default::default()
    };

    let mut spotify = AuthCodePkceSpotify::new(creds.clone(), oauth.clone());
    spotify.config.token_cached = true;
    spotify.config.cache_path = app_config_dir().join(TOKEN_CACHE_FILE);

    let auth_url = spotify.get_authorize_url(None)?;
    let _ = get_token(&mut spotify, &auth_url).await?;
    
    Ok(spotify)
}
