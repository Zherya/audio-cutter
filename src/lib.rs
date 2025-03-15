pub mod audio_cutter_app;
mod audio_thread;

/// Audio source: decoded audio data.
///
/// Any type, that can represent audio data, has to implement [rodio::Source] trait, which is an
/// [Iterator] over audio samples. Such type can be, for example, [rodio::Decoder] that can decode
/// audio samples data from a file or any other input that implement Read and Seek traits.
///
/// We will read audio data from files only, so use [rodio::Decoder] on files as main audio source.
type DecodedAudioSource = rodio::Decoder<std::io::BufReader<std::fs::File>>;
/// Buffered audio source.
///
/// In addition, we wrap [DecodedAudioSource] into [rodio::source::Buffered] struct, as it buffers
/// audio source data and can be cloned, so we decode audio data only once, even if playing it
/// multiple times.
type AudioSourceBuf = rodio::source::Buffered<DecodedAudioSource>;
