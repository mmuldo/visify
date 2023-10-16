use std::{sync::Arc, process::exit};

use visualizer::{show, auth::auth};

#[tokio::main]
async fn main() {
    let client = Arc::new(match auth().await {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Failed to authenticate with spotify: {error}");
            exit(1);
        }
    });

    match show(client) {
        Ok(_) => (),
        Err(error) => {
            eprintln!("GUI error: {error}");
            exit(1);
        }
    }
}
