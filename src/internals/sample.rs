use anyhow::anyhow;
use std::{fs, path::PathBuf};
use symphonia::core::audio::sample::Sample;
use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    formats::{FormatOptions, FormatReader, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::{Time, Timestamp},
};

pub struct SampleHandle<'mss> {
    pub path: PathBuf,
    pub mss: MediaSourceStream<'mss>,
}

impl<'mss> SampleHandle<'mss> {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let file = fs::File::open(&path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        Ok(Self { path, mss })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SampleProperties {
    pub sample_rate: u32,
    pub length: i128,
}

impl SampleProperties {
    pub fn length(&self) -> Time {
        Time::from_millis(self.length as i64)
    }
}

pub fn fetch_sample_properties(path: &PathBuf) -> anyhow::Result<SampleProperties> {
    let file = fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let probed: Box<dyn FormatReader> = symphonia::default::get_probe().probe(
        &Hint::new(),
        mss,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;

    let media_info = probed.media_info();

    let default_track = probed
        .default_track(symphonia::core::formats::TrackType::Audio)
        .ok_or(anyhow!("No default track set for file."))?;

    let codec_params = default_track.codec_params.as_ref().ok_or(anyhow!(
        "Format reader unable to determine codec parameters."
    ))?;

    let audio_params = codec_params.audio().ok_or(anyhow!("Unsupported file."))?;

    let sample_duration = if let Some(dur) = media_info.duration
        && let Some(tbase) = media_info.time_base
    {
        Ok(tbase.calc_time_saturating(Timestamp::new(dur.get() as i64)))
    } else {
        Err(anyhow!("Could not determine length due to missing header."))
    }?;

    Ok(SampleProperties {
        sample_rate: audio_params.sample_rate.unwrap_or_default(),
        length: sample_duration.as_millis(),
    })
}

pub fn generate_sample_waveform(path: &PathBuf) -> anyhow::Result<Vec<[f32; 2]>> {
    let file = fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Use the default options when reading and decoding.
    let fmt_opts: FormatOptions = Default::default();
    let meta_opts: MetadataOptions = Default::default();
    let dec_opts: AudioDecoderOptions = Default::default();

    let mut probe =
        symphonia::default::get_probe().probe(&Hint::new(), mss, fmt_opts, meta_opts)?;

    let default_track = probe
        .default_track(symphonia::core::formats::TrackType::Audio)
        .ok_or(anyhow!("No default track set for file."))?;

    let codec_params = default_track.codec_params.as_ref().ok_or(anyhow!(
        "Format reader unable to determine codec parameters."
    ))?;

    let audio_params = codec_params.audio().ok_or(anyhow!("Unsupported file."))?;

    let mut decoder =
        symphonia::default::get_codecs().make_audio_decoder(audio_params, &dec_opts)?;

    let track_id = default_track.id;

    let mut samples: Vec<f32> = Vec::default();
    let mut channel_count = 0;
    let mut packet_buf: Vec<f32> = Vec::new();

    while let Some(packet) = probe.next_packet()? {
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                if channel_count == 0 {
                    channel_count = audio_buf.spec().channels().count();
                }
                packet_buf.resize(audio_buf.samples_interleaved(), f32::MID);
                audio_buf.copy_to_slice_interleaved(&mut packet_buf);
                samples.extend_from_slice(&packet_buf); // append, don't overwrite
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    let avg_channel_input = avg_values_in_window(&samples, channel_count);

    let value_pairs = min_max_in_window(&avg_channel_input, 1024);

    return Ok(value_pairs);
}

fn avg_values_in_window(input: &[f32], window_size: usize) -> Vec<f32> {
    let mut output = Vec::new();
    let mut idx = 0;

    while idx < input.len() {
        let end = (idx + window_size).min(input.len());

        // Take all of the channels' signal and avg it down into one number
        let items = &input[idx..end];

        output.push(items.iter().sum::<f32>() / items.len() as f32);

        idx = end;
    }

    output
}

fn min_max_in_window(input: &[f32], window_size: usize) -> Vec<[f32; 2]> {
    let mut output = Vec::new();

    let mut idx = 0;

    while idx < input.len() {
        let end = (idx + window_size).min(input.len());

        let items = &input[idx..end];

        let min = items.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = items.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        output.push([min, max]);

        idx = end;
    }

    output
}
