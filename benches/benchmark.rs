use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dancey::{MusicGrid, SoundNode};

fn bench_create_preview_samples(c: &mut Criterion) {
    let mut music_grid = MusicGrid::new(10, None);
    *music_grid.beat_per_minute_mut() = 100;
    
    let sound_node = SoundNode::new("soundnode1".to_string(), 1, PathBuf::from("benches\\sounds\\1.mp3"), 48000).unwrap();
    let sound_node1 = SoundNode::new("soundnode2".to_string(), 1, PathBuf::from("benches\\sounds\\2.mp3"), 48000).unwrap();
    let sound_node2 = SoundNode::new("soundnode3".to_string(), 1, PathBuf::from("benches\\sounds\\3.mp3"), 48000).unwrap();
    let sound_node3 = SoundNode::new("soundnode4".to_string(), 1, PathBuf::from("benches\\sounds\\4.mp3"), 48000).unwrap();

    music_grid.insert_node(1, sound_node);
    music_grid.insert_node(2, sound_node1);
    music_grid.insert_node(3, sound_node2);
    music_grid.insert_node(4, sound_node3);

    c.bench_function("create_preview_samples (Non-SIMD)", |b| {
        b.iter(|| black_box(music_grid.create_preview_samples()))
    });

    c.bench_function("create_preview_samples_simd (SIMD)", |b| {
        b.iter(|| black_box(music_grid.create_preview_samples_simd()))
    });
}

criterion_group!(benches, bench_create_preview_samples);
criterion_main!(benches);