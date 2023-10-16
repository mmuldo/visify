pub mod auth;
pub mod config;
pub mod spectrum;
pub mod state;
use std::{sync::Arc, thread, result, time};
use chrono;

use eframe::{egui::{self, Vec2, FontDefinitions}, run_native, CreationContext, NativeOptions, App, Frame, emath::Numeric, epaint::{Color32, FontFamily, FontId}, Storage};
use rspotify::{AuthCodePkceSpotify, prelude::OAuthClient, model::{AdditionalType, PlayableItem, RepeatState}, ClientError};
use spectrum::Bode;
use tokio::sync::mpsc::{channel, Sender, Receiver};
use state::{State, StateResult, StateError, Client};

struct Visualizer {
    state: State,
    bode: Bode,
    rx: Receiver<StateResult<State>>
}

impl Visualizer {
    fn new(rx: Receiver<StateResult<State>>) -> Self {
        Self {
            state: State::default(),
            bode: Bode::new(),
            rx
        }
    }
}

impl App for Visualizer {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        let try_state = self.rx.try_recv();
        if let Ok(Ok(state)) = try_state {
            self.state = state;
        } else if let Ok(Err(error)) = try_state{
            eprintln!("{error}");
        }

        let frame_width = frame.info().window_info.size.x;
        let frame_height = frame.info().window_info.size.y;

        egui::TopBottomPanel::bottom("spectrum")
            .show_separator_line(false)
            .exact_height(frame_height * 0.4)
            .show(ctx, |ui| {
                self.bode.show(ui);
            });

        egui::TopBottomPanel::bottom("progress_bar")
            .show_separator_line(false)
            .exact_height(frame_height * 0.1)
            .show(ctx, |ui| {
                let progress = self.state.progress + chrono::Duration::from_std(self.state.instant_of_last_refresh.elapsed()).unwrap_or(chrono::Duration::zero());
                let progress_bar = egui::ProgressBar::new(
                    progress.num_milliseconds() as f32 / self.state.duration.num_milliseconds() as f32
                )
                    .text(format!("{} / {}", format_duration(progress), format_duration(self.state.duration)))
                    .fill(Color32::from_rgb(122, 36, 39));
                ui.add(progress_bar);
            });

        egui::CentralPanel::default()
            .show(ctx, |ui| {
                let info_layout = egui::Layout::top_down(eframe::emath::Align::Center);

                let panel_height = frame_height * 0.5;

                egui::SidePanel::left("track_info")
                    .show_separator_line(false)
                    .exact_width(frame_width / 3.)
                    .show(ctx, |ui| {
                    ui.with_layout(info_layout, |ui| {
                        ui.add_space(panel_height * 0.1);
                        let track = egui::RichText::new(format!("{}", self.state.track)).size(panel_height * 0.10);
                        ui.label(track);

                        ui.add_space(panel_height * 0.1);
                        let album = egui::RichText::new(format!("{}", self.state.album)).size(panel_height * 0.05);
                        ui.label(album);

                        ui.add_space(panel_height * 0.1);
                        let artists = egui::RichText::new(format!("{}", self.state.artists.join(", "))).size(panel_height * 0.075);
                        ui.label(artists);
                    });
                });

                let icons_layout = egui::Layout::top_down(eframe::emath::Align::Center);
                egui::SidePanel::right("icons")
                    .show_separator_line(false)
                    .exact_width(frame_width / 3.)
                    .show(ctx, |ui| {
                        ui.with_layout(icons_layout, |ui| {
                            let active_color = Color32::from_rgb(196, 39, 39);
                            let inactive_color = Color32::from_rgb(156, 116, 116);

                            ui.add_space(panel_height * 0.1);
                            let liked = egui::RichText::new("")
                                .font(FontId::new(panel_height * 0.1, FontFamily::Proportional))
                                .color(if self.state.liked {active_color} else {inactive_color});
                            ui.label(liked);

                            ui.add_space(panel_height * 0.1);
                            let shuffled = egui::RichText::new("")
                                .font(FontId::new(panel_height * 0.1, FontFamily::Proportional))
                                .color(if self.state.shuffled {active_color} else {inactive_color});
                            ui.label(shuffled);

                            ui.add_space(panel_height * 0.1);
                            let (repeat_glyph, repeat_color) = match self.state.repeat_state {
                                RepeatState::Off => ("", inactive_color),
                                RepeatState::Context => ("", active_color),
                                RepeatState::Track => ("", active_color)
                            };

                            let repeat_state = egui::RichText::new(repeat_glyph)
                                .font(FontId::new(panel_height * 0.1, FontFamily::Proportional))
                                .color(repeat_color);
                            ui.label(repeat_state);
                        });
                });

                egui::CentralPanel::default()
                    .show(ctx, |ui| {
                        let image = egui::Image::new(self.state.cover_art_url.clone());
                        ui.add(image);
                    })
            });

        ctx.request_repaint();
    }
}

fn format_two_digit_int(number: i64) -> String {
    let tens = number.div_euclid(10);
    let ones = number % 10;

    format!("{}{}", tens.to_string(), ones.to_string())
}

fn format_duration(duration: chrono::Duration) -> String {
    let minutes = duration.num_minutes();
    let seconds = duration.num_seconds() % 60;
    format!("{}:{}",
    format_two_digit_int(minutes),
    format_two_digit_int(seconds)
)
}

pub fn show(client: Arc<AuthCodePkceSpotify>) -> eframe::Result<()> {
    let (tx, rx) = channel(1);
    let client = Client::new(client, tx);
    let visualizer = Visualizer::new(rx);

    client.spawn();

    let mut native_options = NativeOptions::default();
    native_options.initial_window_size = Some(Vec2::new(750., 500.));
    native_options.max_window_size = Some(Vec2::new(750., 500.));

    run_native(
        "Rofify Visualizer",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let mut fonts = FontDefinitions::default();
            fonts.font_data.insert(
                "awesome".to_owned(),
                egui::FontData::from_static(include_bytes!("font-awesome-solid.ttf"))
            );
            fonts.families.get_mut(&FontFamily::Proportional)
                .unwrap()
                .push("awesome".to_owned());
            cc.egui_ctx.set_fonts(fonts);

            Box::new(visualizer)
        })
    )
}
