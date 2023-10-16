use rspotify::{AuthCodePkceSpotify, prelude::OAuthClient, model::{AdditionalType, PlayableItem, RepeatState}, ClientError};
use std::{sync::Arc, thread, result, time::{self, Instant}};
use chrono;
use tokio::sync::mpsc::{channel, Sender, Receiver};

pub const REFRESH_RATE_MS: u64 = 5000;

#[derive(thiserror::Error, Debug)]
pub enum StateError {
    #[error("Error from spotify client: {0}")]
    Client(#[from] ClientError),
    #[error("Nothing is playing at the moment.")]
    NoContext,
    #[error("Could not get some of the required state from the client.")]
    MissingState,
}

pub type StateResult<T> = result::Result<T, StateError>;

pub struct State {
    pub liked: bool,
    pub shuffled: bool,
    pub repeat_state: RepeatState,
    pub progress: chrono::Duration,
    pub duration: chrono::Duration,
    pub instant_of_last_refresh: Instant,
    pub track: String,
    pub album: String,
    pub artists: Vec<String>,
    pub cover_art_url: String,
}

impl Default for State {
    fn default() -> Self {
        State {
            liked: Default::default(),
            shuffled: Default::default(),
            repeat_state: RepeatState::Off,
            progress: chrono::Duration::seconds(0),
            duration: chrono::Duration::seconds(0),
            instant_of_last_refresh: Instant::now(),
            track: Default::default(),
            album: Default::default(),
            artists: Default::default(),
            cover_art_url: Default::default(),
        }
    }
}

pub struct Client {
    pub client: Arc<AuthCodePkceSpotify>,
    pub tx: Sender<StateResult<State>>
}

impl Client {
    pub fn new(client: Arc<AuthCodePkceSpotify>, tx: Sender<StateResult<State>>) -> Self {
        Self {
            client,
            tx
        }
    }

    async fn get_state(&self) -> StateResult<State>{
        if let Some(current_playback_context) = self.client.current_playback(None, Some([
            &AdditionalType::Track,
            &AdditionalType::Episode
        ])).await? {
            if let (Some(progress), Some(PlayableItem::Track(track))) = (current_playback_context.progress, current_playback_context.item) {
                let liked = self.client
                    .current_user_saved_tracks_contains([track.id.clone().unwrap()])
                    .await?
                    .first()
                    .unwrap()
                    .clone();
                let shuffled = current_playback_context.shuffle_state;
                let repeat_state = current_playback_context.repeat_state;

                let duration = track.duration;
                let instant_of_last_refresh = Instant::now();

                let track_name = track.name.clone();
                let album = track.album.name.clone();
                let artists: Vec<String> = track.artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect();

                let cover_art_url = track.album.images.first().unwrap().url.clone();

                Ok(State {
                    liked,
                    shuffled,
                    repeat_state,
                    progress,
                    duration,
                    instant_of_last_refresh,
                    track: track_name,
                    album,
                    artists,
                    cover_art_url,
                })
            } else {
                Err(StateError::MissingState)
            }
        } else {
            Err(StateError::NoContext)
        }
    }

    pub fn spawn(self) {
        tokio::spawn(async move {
            while let Ok(()) = self.tx.send(self.get_state().await).await {
                tokio::time::sleep(time::Duration::from_millis(REFRESH_RATE_MS)).await;
            }
        });
    }
}

