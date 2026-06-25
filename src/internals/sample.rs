use anyhow::anyhow;
use std::{fs, path::PathBuf};
use symphonia::core::{
    formats::{FormatOptions, FormatReader, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::{Time, Timestamp},
};

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

    let sample_duration = if let Some(dur) = media_info.duration
        && let Some(tbase) = media_info.time_base
    {
        tbase.calc_time_saturating(Timestamp::new(dur.get() as i64))
    }
    // Fallback calculation
    else {
        probed
            .default_track(symphonia::core::formats::TrackType::Audio)
            .ok_or(anyhow!("No default track set for file."))?;

        todo!()
    };

    Ok(SampleProperties {
        sample_rate: 235523,
        length: sample_duration.as_millis(),
    })
}
