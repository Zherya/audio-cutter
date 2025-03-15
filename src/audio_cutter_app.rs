use crate::audio_thread;
use crate::audio_thread::AudioControlCommand;
use eframe::egui;
use rodio::Source;
use std::fs::File;

/// Current audio playback status.
enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

/// AudioCutterApp controls application UI.
pub struct AudioCutterApp {
    playback_status: PlaybackStatus,

    /// AudioThread controls separate thread that performs audio playback process.
    audio_thread: Option<audio_thread::AudioThread>,

    /// Current audio track filename, chosen by user.
    current_file_name: Option<std::path::PathBuf>,
    /// Audio source that corresponds to current audio track.
    audio_source: Option<crate::AudioSourceBuf>,
}

impl AudioCutterApp {
    /// Loads audio source of the current audio track.
    ///
    /// # Panics
    ///
    /// Panics if there is no current audio track, i.e. `current_file_name` is [None].
    fn load_audio_source(&mut self) {
        let file = File::open(self.current_file_name.as_ref().unwrap()).unwrap();

        // TODO: Handle decoding error
        let audio_source = rodio::Decoder::new(std::io::BufReader::new(file)).unwrap();
        self.audio_source = Option::from(audio_source.buffered());
    }

    /// Controls behavior of opening file UI button.
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
                // Stop playing current audio, if new file is chosen
                self.audio_thread
                    .as_ref()
                    .unwrap()
                    .send(AudioControlCommand::Stop)
                    .unwrap();

                self.current_file_name = Some(file);
                println!("[Audio Cutter App] Loading audio source ...");
                self.load_audio_source();
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
            // horizontally, when already center horizontally other widgets from top to down
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
            }

            // Button in the widget for playing and pausing
            if ui.button(action).clicked() {
                match self.playback_status {
                    PlaybackStatus::Playing => {
                        // TODO: Handle Result
                        self.audio_thread
                            .as_ref()
                            .unwrap()
                            .send(AudioControlCommand::Pause)
                            .unwrap();
                        self.playback_status = PlaybackStatus::Paused;
                    }
                    PlaybackStatus::Paused => {
                        // TODO: Handle Result
                        self.audio_thread
                            .as_ref()
                            .unwrap()
                            .send(AudioControlCommand::Continue)
                            .unwrap();
                        self.playback_status = PlaybackStatus::Playing;
                    }
                    PlaybackStatus::Stopped => {
                        println!("[Audio Cutter App] Sending Play command ...");
                        self.audio_thread
                            .as_ref()
                            .unwrap()
                            .send(AudioControlCommand::Play(
                                self.audio_source.as_ref().unwrap().clone(),
                            ))
                            .unwrap();
                        self.playback_status = PlaybackStatus::Playing;
                    }
                }
            }
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

                    self.playback_control(ui);

                    // Print elapsed time
                    let elapsed_duration = self.audio_thread.as_ref().unwrap().time_elapsed();
                    let minutes = (elapsed_duration.as_secs() / 60).to_string();
                    let seconds = (elapsed_duration.as_secs() % 60).to_string();
                    ui.label(minutes + ":" + seconds.as_str());
                }
            });
        });
    }
}
