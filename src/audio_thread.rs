use eframe::egui;
use std::sync::mpsc::{SendError, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Commands to control a thread, that performs audio playback.
pub enum AudioControlCommand {
    /// Play command to start new playback with new audio source.
    Play(crate::AudioSourceBuf),
    Pause,
    Continue,
    Stop,
}

/// Struct that owns and controls a thread, that performs audio playback process.
pub struct AudioThread {
    /// Thread handle to a thread, that performs audio playback.
    ///
    /// Handle is wrapped in [Option] for graceful joining, when [AudioThread] is dropped.
    thread_handle: Option<std::thread::JoinHandle<()>>,
    time_elapsed: Arc<Mutex<Duration>>,
    commands_sender: Option<std::sync::mpsc::Sender<AudioControlCommand>>,
}

impl AudioThread {
    /// Creates new [AudioThread] object with a spawned audio thread.
    ///
    /// # Parameters
    ///
    /// * `ui_ctx` - UI context handle, used by audio playback thread to force UI repainting.
    ///
    /// # Panics
    ///
    /// Panics if the OS fails to create a thread.
    pub fn spawn(ui_ctx: &egui::Context) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let time_elapsed = Arc::new(Mutex::new(Duration::ZERO));

        let thread_ctx = ThreadContext {
            commands_receiver: receiver,
            time_elapsed: Arc::clone(&time_elapsed),
            ui_ctx: ui_ctx.clone(),
        };

        let thread_handle = std::thread::spawn(move || {
            playback_audio(thread_ctx);
        });

        Self {
            thread_handle: Option::from(thread_handle),
            time_elapsed,
            commands_sender: Option::from(sender),
        }
    }

    /// Sends a command to the audio playback thread.
    ///
    /// # Parameters
    ///
    /// * `command` - the command to send to the audio playback thread.
    pub fn send(&self, command: AudioControlCommand) -> Result<(), SendError<AudioControlCommand>> {
        self.commands_sender.as_ref().unwrap().send(command)
    }

    /// Returns the current duration of audio track elapsed time.
    pub fn time_elapsed(&self) -> Duration {
        *self.time_elapsed.lock().unwrap()
    }
}

impl Drop for AudioThread {
    fn drop(&mut self) {
        // Take sender end of the channel out of Option and then drop it for notifying the audio
        // thread about the stop. take() is needed as drop() takes mutable reference to self, but
        // dropping requires moving, so we leave None in commands_sender field of the self here
        drop(self.commands_sender.take());

        // As with commands sender, take the ownership over the audio thread handle and then join it
        if let Some(thread) = self.thread_handle.take() {
            thread.join().unwrap();
        }
    }
}

/// Struct that stores playback context data, controlled by the audio playback thread.
struct ThreadContext {
    commands_receiver: std::sync::mpsc::Receiver<AudioControlCommand>,
    time_elapsed: Arc<Mutex<Duration>>,
    ui_ctx: egui::Context,
}

/// Entry point for the audio playback thread.
///
/// # Parameters
///
/// * `thread_ctx` - playback context data, controlled by the audio playback thread.
fn playback_audio(thread_ctx: ThreadContext) {
    // For default physical audio device, create output stream and more useful handle to that
    // stream. Audio stream must exist or playback will end and attached handle will no longer
    // work
    let (_audio_stream, audio_stream_handle) = rodio::OutputStream::try_default().unwrap();

    // Sink is a handle for easier playback control and represents audio track.
    //
    // In fact, rodio itself spawns a background thread that is dedicated to reading from the
    // sources and sending the output to the device. Whenever you give up ownership of a Source
    // in order to play it, it is sent to this background thread where it will be read by rodio.
    //
    // So we may not use our own separate thread that controls Sink, but will do it in order to
    // separate functionality and achieve some sort of modularity.
    // TODO: If we place Sink in main thread we will not able to update elapsed time, when no
    // TODO: actions are performed on the UI, right? As update() will not be called then. So
    // TODO: separate thread is needed anyway
    let audio_sink = rodio::Sink::try_new(&audio_stream_handle).unwrap();

    loop {
        if audio_sink.empty() || audio_sink.is_paused() {
            println!("[Audio Thread] recv() ...");
            // If no sound is currently playing we can use blocking wait for new command in
            // order to save CPU time
            if let Ok(command) = thread_ctx.commands_receiver.recv() {
                handle_command(&thread_ctx, command, &audio_sink);
                continue;
            } else {
                // Disconnected
                return;
            }
        }

        // Otherwise sound is playing, and we have to handle new command or update elapsed time
        // without blocking
        match thread_ctx.commands_receiver.try_recv() {
            Ok(command) => handle_command(&thread_ctx, command, &audio_sink),
            Err(error) => {
                if let TryRecvError::Disconnected = error {
                    return;
                }

                // No commands yet: update elapsed time of the audio
                *thread_ctx.time_elapsed.lock().unwrap() = audio_sink.get_pos();
                // Force UI repainting to show new elapsed time
                thread_ctx.ui_ctx.request_repaint();
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Handles single received audio control command.
///
/// # Parameters
///
/// * `thread_ctx` - playback context data, controlled by the audio playback thread.
/// * `command` - the command to handle.
/// * `audio_sink` - [rodio::Sink] that actually performs audio playback.
fn handle_command(
    thread_ctx: &ThreadContext,
    command: AudioControlCommand,
    audio_sink: &rodio::Sink,
) {
    match command {
        AudioControlCommand::Play(audio_source) => {
            // Pauses playback and remove all loaded audio sources.
            // Note that stop() should not be used generally, as sink shouldn't be used after
            // stop(): https://github.com/RustAudio/rodio/issues/171
            audio_sink.clear();
            // The sound starts playing in the separate thread, controlled by the sink, once
            // some data is appended to the sink, if it is not paused
            audio_sink.append(audio_source);
            audio_sink.play();
        }
        AudioControlCommand::Pause => audio_sink.pause(),
        AudioControlCommand::Continue => audio_sink.play(),
        AudioControlCommand::Stop => {
            audio_sink.clear();
            // Also clear elapsed time of the audio
            *thread_ctx.time_elapsed.lock().unwrap() = Duration::ZERO;
            // Force UI repainting to show new elapsed time
            thread_ctx.ui_ctx.request_repaint();
        }
    }
}
