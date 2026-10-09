//! Optional sound discovery through Bevy's asset reader, without submitting
//! absent alternatives as failed AssetServer loads. Uses the configured native
//! asset root or browser reader; other asset errors remain visible.
use super::{AudioSource, decode};
use bevy::{
    asset::{
        AssetServer,
        io::{AssetReaderError, AssetSourceId, ErasedAssetReader},
    },
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use std::path::Path;

async fn clip(
    reader: &dyn ErasedAssetReader,
    base: &str,
    formats: [&str; 2],
) -> Option<AudioSource> {
    for ext in formats {
        let path = format!("sounds/{base}.{ext}");
        let mut file = match reader.read(Path::new(&path)).await {
            Ok(file) => file,
            Err(AssetReaderError::NotFound(_)) => continue,
            Err(error) => {
                warn!("could not read sound {path}: {error}");
                continue;
            }
        };
        let mut bytes = Vec::new();
        if let Err(error) = file.read_to_end(&mut bytes).await {
            warn!("could not read sound {path}: {error}");
            continue;
        }
        let source = AudioSource {
            bytes: bytes.into(),
        };
        match decode(source.clone()) {
            Ok(_) => return Some(source),
            Err(error) => warn!("could not decode sound {path}: {error}"),
        }
    }
    None
}
async fn variants(reader: &dyn ErasedAssetReader, name: &str) -> Vec<AudioSource> {
    if let Some(source) = clip(reader, name, ["ogg", "wav"]).await {
        return vec![source];
    }
    let mut sources = Vec::new();
    while let Some(source) =
        clip(reader, &format!("{name}_{}", sources.len()), ["ogg", "wav"]).await
    {
        sources.push(source);
    }
    sources
}
pub(super) fn hit(assets: AssetServer, name: &'static str) -> Task<Vec<AudioSource>> {
    IoTaskPool::get().spawn(async move {
        let Ok(source) = assets.get_source(AssetSourceId::Default) else {
            warn!("sound asset source is unavailable");
            return Vec::new();
        };
        let result = variants(source.reader(), name).await;
        if result.is_empty() {
            warn!("sound {name} is unavailable; leaving it silent");
        }
        result
    })
}
pub(super) fn track(assets: AssetServer, name: &'static str) -> Task<Option<AudioSource>> {
    IoTaskPool::get().spawn(async move {
        let Ok(source) = assets.get_source(AssetSourceId::Default) else {
            warn!("sound asset source is unavailable");
            return None;
        };
        clip(source.reader(), name, ["wav", "ogg"]).await
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::io::memory::{Dir, MemoryAssetReader},
        tasks::block_on,
    };
    fn file(dir: &Dir, path: &str, sample: i16) -> Vec<u8> {
        let mut bytes = super::super::tests::wave().bytes.to_vec();
        bytes[44..46].copy_from_slice(&sample.to_le_bytes());
        dir.insert_asset(Path::new(path), bytes.clone());
        bytes
    }
    #[test]
    fn optional_formats_fall_back_and_bare_name_wins() {
        let dir = Dir::default();
        let wav = file(&dir, "sounds/cloth.wav", 200);
        file(&dir, "sounds/cloth_0.wav", 100);
        let reader = MemoryAssetReader { root: dir.clone() };
        let found = block_on(variants(&reader, "cloth"));
        assert_eq!(found.len(), 1);
        assert_eq!(&*found[0].bytes, wav);
        let ogg = file(&dir, "sounds/cloth.ogg", 300);
        assert_eq!(&*block_on(variants(&reader, "cloth"))[0].bytes, ogg);
        // Loops reverse the extension priority. Decoder detects the actual
        // container; the different bytes identify the selected candidate.
        assert_eq!(
            &*block_on(clip(&reader, "cloth", ["wav", "ogg"]))
                .unwrap()
                .bytes,
            wav
        );
    }
    #[test]
    fn missing_and_invalid_alternatives_stop_at_first_variant_gap() {
        let dir = Dir::default();
        dir.insert_asset_text(Path::new("sounds/step_0.ogg"), "not audio");
        let first = file(&dir, "sounds/step_0.wav", 100);
        let second = file(&dir, "sounds/step_1.ogg", 200);
        file(&dir, "sounds/step_3.wav", 300);
        let reader = MemoryAssetReader { root: dir };
        let found = block_on(variants(&reader, "step"));
        assert_eq!(found.len(), 2);
        assert_eq!(&*found[0].bytes, first);
        assert_eq!(&*found[1].bytes, second);
        assert!(block_on(variants(&reader, "absent")).is_empty());
        assert!(block_on(clip(&reader, "absent", ["wav", "ogg"])).is_none());
    }
}
