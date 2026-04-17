use crate::audio_thread::{self, AudioControlCommand};
use eframe::egui::{self, containers::Frame, emath, epaint, epaint::PathStroke};
use rodio::Source;
use std::fs::File;

/// Current audio playback status.
enum PlaybackStatus {
    Playing,
    Stopped,
}

/// AudioCutterApp controls application UI.
pub struct AudioCutterApp {
    playback_status: PlaybackStatus,

    /// AudioThread controls separate thread that performs audio playback process.
    audio_thread: Option<audio_thread::AudioThread>,

    /// Current audio track filename, chosen by user.
    current_file_name: Option<std::path::PathBuf>,
    /// Audio source that corresponds to the current audio track.
    audio_source: Option<crate::AudioSourceBuf>,
    samples: Vec<f32>,
    max_sample: f32,
    /// Duration of the part of the audio_source, that user skips before sending an audio source
    /// to the AudioThread.
    skipped_from_beg: std::time::Duration,
    /// Position of the currently chosen or playing sample on the audio wave.
    audio_wave_position: f32,
}

impl AudioCutterApp {
    /// Loads audio source of the current audio track.
    ///
    /// # Panics
    ///
    /// Panics if there is no current audio track, i.e. `current_file_name` is [None].
    fn load_audio_source(&mut self) {
        println!(
            "[Audio Cutter App] Loading audio source: {}...",
            self.current_file_name.as_ref().unwrap().display()
        );
        let file = File::open(self.current_file_name.as_ref().unwrap()).unwrap();

        // TODO: Handle decoding error
        let audio_source = rodio::Decoder::new(std::io::BufReader::new(file)).unwrap();
        self.audio_source = Option::from(audio_source.buffered());
    }

    fn load_samples(&mut self) {
        // Number of samples per second
        let rate = self.audio_source.as_ref().unwrap().sample_rate();
        println!("[Audio Cutter App] Samples rate: {}", rate);

        println!(
            "[Audio Cutter App] Channels: {}",
            self.audio_source.as_ref().unwrap().channels()
        );

        let samples = self.audio_source.as_ref().unwrap().clone();
        let samples: Vec<f32> = samples.collect();
        println!("[Audio Cutter App] Samples length: {}", samples.len());

        // We take only positive samples for simpler sound wave
        let samples: Vec<f32> = samples.into_iter().filter(|&s| s >= 0.0).collect();

        // Fold samples to only such a number of values, that corresponds to seconds
        let samples: Vec<f32> = samples
            .chunks(rate as usize)
            .map(|chunk| chunk.iter().fold(0.0, |acc, x| acc + x))
            .collect();

        self.max_sample = samples[0];
        for &sample in &samples {
            if sample > self.max_sample {
                self.max_sample = sample;
            }
        }

        self.samples = samples;
    }

    /// Controls the behavior of opening file UI button.
    ///
    /// # Parameters
    ///
    /// * `ui` - `egui::UI` for placing the button on.
    fn open_file_button(&mut self, ui: &mut egui::Ui) {
        if ui.button("Открыть файл...").clicked() {
            if let Some(file) = rfd::FileDialog::new()
                .add_filter("MP3 файл", &["mp3"])
                .pick_file()
            {
                // Stop playing current audio if a new file is chosen
                self.audio_thread
                    .as_ref()
                    .unwrap()
                    .send(AudioControlCommand::Stop)
                    .unwrap();

                self.current_file_name = Some(file);

                self.load_audio_source();
                self.load_samples();
            }
        }
    }

    /// Controls audio playback part of the UI.
    ///
    /// # Parameters
    ///
    /// * `ui` - `egui::UI` for placing audio playback controls on.
    fn playback_control(&mut self, ui: &mut egui::Ui) {
        let action;
        if let PlaybackStatus::Playing = self.playback_status {
            action = String::from("Пауза");
        } else {
            action = String::from("Играть");
        }

        ui.horizontal(|ui| {
            // Trying to center buttons two buttons in the same row at the center
            // horizontally, when already horizontally center other widgets from top to down
            // might be a problem for immediate mode UI:
            // https://github.com/emilk/egui/discussions/2916
            // Rude fix is to add some constant space before buttons or need to change the
            // UI layout (better)
            // let button_width = 40.0;
            // let total_width = button_width * 2.0;
            // ui.add_space((ui.available_width() - total_width) / 2.0);
            // Button in the widget for stopping
            if ui.button("Стоп").clicked() {
                // TODO: Handle Result
                self.audio_thread
                    .as_ref()
                    .unwrap()
                    .send(AudioControlCommand::Stop)
                    .unwrap();
                self.playback_status = PlaybackStatus::Stopped;
                self.audio_wave_position = 0.0;
                self.skipped_from_beg = std::time::Duration::ZERO;
            }

            // Button in the widget for playing and pausing
            if ui.button(action).clicked() {
                match self.playback_status {
                    PlaybackStatus::Playing => {
                        // Pause is the same as stop, but we don't clear audio wave position and
                        // skipped duration immediately.
                        //
                        // In context of AudioThread, we don't differ stop and pause, as user can
                        // change start time, so we have to send a new audio source to the
                        // AudioThread each time.
                        // TODO: Handle Result
                        self.audio_thread
                            .as_ref()
                            .unwrap()
                            .send(AudioControlCommand::Stop)
                            .unwrap();
                        self.playback_status = PlaybackStatus::Stopped;
                    }
                    PlaybackStatus::Stopped => {
                        self.skipped_from_beg =
                            std::time::Duration::from_secs_f32(self.audio_wave_position);
                        let source = self
                            .audio_source
                            .as_ref()
                            .unwrap()
                            .clone()
                            .skip_duration(self.skipped_from_beg);
                        self.audio_thread
                            .as_ref()
                            .unwrap()
                            .send(AudioControlCommand::Play(source))
                            .unwrap();
                        self.playback_status = PlaybackStatus::Playing;
                    }
                }
            }
        });
    }

    // TODO: use epaint as dancing strings demo?
    // TODO: look at https://github.com/Cannedfood/egui-audio/tree/main

    fn paint_sound_wave(&self, ui: &mut egui::Ui) {
        if self.audio_source.is_none() {
            return;
        }

        Frame::window(ui.style()).show(ui, |ui| {
            ui.ctx().request_repaint();

            // Desired size of the frame with sound wave: occupy all available width (x-coordinate)
            // and use 25% of the width as height (y-coordinate)
            let desired_size = ui.available_width() * egui::vec2(1.0, 0.25);
            let (_id, frame_rect) = ui.allocate_space(desired_size);

            // Linear transformation from the rectangle with audio samples bars to frame rectangle
            let to_screen = emath::RectTransform::from_to(
                egui::Rect::from_x_y_ranges(0.0..=self.samples.len() as f32, 0.0..=self.max_sample),
                frame_rect,
            );

            let mut sample_bars = vec![];

            for (second, &sample) in self.samples.iter().enumerate() {
                let mut points = vec![];

                // Egui uses a coordinate system, where the left-top corner of the screen is
                // (0.0, 0.0), with X increasing to the right and Y increasing downwards. So we have
                // to use maximum sample value (Y) as the bottom of the samples bars, otherwise bars
                // will be inverted
                points.push(to_screen * egui::pos2(second as f32, self.max_sample));
                points.push(to_screen * egui::pos2(second as f32, self.max_sample - sample));

                let thickness = 2.0;
                let path_stroke;
                if (second as f32) < self.audio_wave_position {
                    // The current second is less than elapsed second, so it is green as "completed"
                    path_stroke = PathStroke::new(thickness, egui::Color32::from_rgb(87, 168, 50));
                } else {
                    path_stroke = PathStroke::new(thickness, egui::Color32::from_rgb(168, 64, 50))
                }

                sample_bars.push(epaint::Shape::line(points, path_stroke));
            }

            ui.painter().extend(sample_bars);
        });
    }
}

impl Default for AudioCutterApp {
    fn default() -> Self {
        Self {
            playback_status: PlaybackStatus::Stopped,
            audio_thread: None,
            current_file_name: None,
            audio_source: None,
            samples: Vec::new(),
            max_sample: 0.0,
            skipped_from_beg: std::time::Duration::ZERO,
            audio_wave_position: 0.0,
        }
    }
}

impl eframe::App for AudioCutterApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // No audio thread launched yet, start it:
        if self.audio_thread.is_none() {
            println!("[Audio Cutter App] Spawning audio thread ...");
            self.audio_thread = Some(audio_thread::AudioThread::spawn(ctx));
        }

        // TODO: Do we need egui::Windows for window resizing? It is not native OS window,
        // but a egui windows that is placed inside native
        // TODO: Place each widget handling in a separate method
        // When UI is updated, we show the following:
        egui::CentralPanel::default().show(ctx, |ui| {
            // New widget in the center:
            ui.vertical_centered(|ui| {
                // Widget heading:
                ui.heading("Audio Cutter");

                self.open_file_button(ui);

                if let Some(picked_file) = &self.current_file_name {
                    ui.horizontal(|ui| {
                        ui.label("Открытый файл:");
                        ui.label(picked_file.file_name().unwrap().to_str().unwrap());
                    });

                    match self.playback_status {
                        PlaybackStatus::Playing => {
                            self.audio_wave_position = self.skipped_from_beg.as_secs_f32()
                                + self
                                .audio_thread
                                .as_ref()
                                .unwrap()
                                .time_elapsed()
                                .as_secs_f32();
                        }
                        _ => {}
                    }

                    self.playback_control(ui);

                    self.paint_sound_wave(ui);

                    ui.spacing_mut().slider_width = ui.available_width();
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.audio_wave_position,
                                0.0..=self.samples.len() as f32,
                            )
                                .show_value(false),
                        )
                        .changed()
                    {
                        ctx.request_repaint();
                        // If audio wave position is changed with slider, start playing from the new
                        // position, if we are playing currently
                        // TODO: Extract sending Play to separate method
                        if let PlaybackStatus::Playing = self.playback_status {
                            self.skipped_from_beg =
                                std::time::Duration::from_secs_f32(self.audio_wave_position);
                            let source = self
                                .audio_source
                                .as_ref()
                                .unwrap()
                                .clone()
                                .skip_duration(self.skipped_from_beg);
                            self.audio_thread
                                .as_ref()
                                .unwrap()
                                .send(AudioControlCommand::Play(source))
                                .unwrap();
                            self.playback_status = PlaybackStatus::Playing;
                        }
                    }

                    // Print elapsed time as minutes and seconds with two digits minimum (00:00)
                    let elapsed_duration =
                        std::time::Duration::from_secs_f32(self.audio_wave_position);
                    let elapsed_duration = format!(
                        "{:02}:{:02}",
                        elapsed_duration.as_secs() / 60,
                        elapsed_duration.as_secs() % 60
                    );
                    ui.label(elapsed_duration);
                }
            });
        });
    }
}
