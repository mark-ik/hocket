//! Durable Hocket project storage over Muniment's host-supplied byte backend.
//!
//! With a [`ZipBackend`](muniment::ZipBackend) the keys here become the entry
//! names of a plain zip archive, so a saved `.hock` file opens in any unzip
//! tool: the manifest is `manifest.cbor` and each captured phrase is an ordinary
//! `media/<hash>.wav`. That openable, importable layout is the no-lock-in
//! project-format doctrine made concrete; the store itself is backend-agnostic.
//!
//! The model's [`ProjectBundle`] is one mutable manifest. Captured media stays
//! immutable and content-addressed under its existing [`MediaRef`], which hashes
//! the capture sample rate and decoded samples together — the WAV file is just a
//! carrier, so the reference is verified against the *decoded* audio on load,
//! not the file bytes.

use std::collections::BTreeSet;
use std::io::Cursor;

use muniment::{Backend, StoreError, WriteOp};
use hocket_model::{MediaRef, PersistenceError, ProjectBundle};

use crate::media::{InMemoryStore, MediaBuffer, MediaStore, hash_buffer};

/// The manifest entry name for one Hocket project archive.
pub const MANIFEST_KEY: &str = "manifest.cbor";
/// Directory prefix for content-addressed media entries: `media/<hash>.wav`.
const MEDIA_PREFIX: &str = "media/";

/// Project storage over a host-selected Muniment backend. A desktop host can
/// use Redb; a browser host can later provide OPFS through the same interface.
pub struct ProjectStore<B> {
    backend: B,
}

/// A manifest plus every media blob that was available at load time. Missing
/// blobs do not prevent opening the project: their layers remain in history but
/// stay silent until a peer, backup, or later import supplies the media.
#[derive(Clone, Debug)]
pub struct LoadedProject {
    pub bundle: ProjectBundle,
    pub media: InMemoryStore,
    pub missing_media: BTreeSet<MediaRef>,
}

#[derive(Debug)]
pub enum ProjectStoreError {
    Store(StoreError),
    Manifest(PersistenceError),
    MissingManifest,
    MissingMedia(BTreeSet<MediaRef>),
    InvalidMedia {
        reference: MediaRef,
        reason: &'static str,
    },
    MediaEncode(String),
    MediaHashMismatch {
        expected: MediaRef,
        actual: MediaRef,
    },
}

impl std::fmt::Display for ProjectStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(error) => write!(f, "storage failed: {error}"),
            Self::Manifest(error) => write!(f, "project manifest failed: {error}"),
            Self::MissingManifest => f.write_str("project manifest is missing"),
            Self::MissingMedia(references) => {
                write!(
                    f,
                    "project save is missing {} media blob(s)",
                    references.len()
                )
            }
            Self::InvalidMedia { reference, reason } => {
                write!(f, "media {reference} is invalid: {reason}")
            }
            Self::MediaEncode(error) => write!(f, "media encoding failed: {error}"),
            Self::MediaHashMismatch { expected, actual } => {
                write!(
                    f,
                    "media hash mismatch: expected {expected}, found {actual}"
                )
            }
        }
    }
}

impl std::error::Error for ProjectStoreError {}

impl From<StoreError> for ProjectStoreError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<PersistenceError> for ProjectStoreError {
    fn from(error: PersistenceError) -> Self {
        Self::Manifest(error)
    }
}

impl<B: Backend> ProjectStore<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Save newly referenced media blobs and the manifest in one backend batch.
    /// Existing blobs are validated and reused. Transactional backends make new
    /// blobs plus the manifest all-or-nothing. Simpler backends may leave only
    /// harmless content-addressed blobs after an interrupted write.
    pub async fn save(
        &self,
        bundle: &ProjectBundle,
        media: &impl MediaStore,
    ) -> Result<(), ProjectStoreError> {
        let references = referenced_media(bundle);
        let missing: BTreeSet<MediaRef> = references
            .iter()
            .filter(|reference| media.get(reference).is_none())
            .copied()
            .collect();
        if !missing.is_empty() {
            return Err(ProjectStoreError::MissingMedia(missing));
        }

        let mut writes = Vec::with_capacity(references.len() + 1);
        for reference in references {
            let buffer = media
                .get(&reference)
                .expect("missing references checked above");
            let actual = hash_buffer(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(ProjectStoreError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
            let key = media_key(reference);
            if let Some(existing) = self.backend.get(&key).await? {
                let stored = decode_media(reference, &existing)?;
                let actual = hash_buffer(&stored.samples, stored.sample_rate);
                if actual != reference {
                    return Err(ProjectStoreError::MediaHashMismatch {
                        expected: reference,
                        actual,
                    });
                }
                continue;
            }
            writes.push(WriteOp::Put {
                key,
                value: encode_media(buffer)?,
            });
        }
        writes.push(WriteOp::Put {
            key: MANIFEST_KEY.to_string(),
            value: bundle.to_bytes()?,
        });
        self.backend.apply(&writes).await?;
        Ok(())
    }

    /// Load the manifest and every available blob. Missing blob keys are
    /// reported in [`LoadedProject::missing_media`] rather than failing open.
    pub async fn load(&self) -> Result<LoadedProject, ProjectStoreError> {
        let bytes = self
            .backend
            .get(MANIFEST_KEY)
            .await?
            .ok_or(ProjectStoreError::MissingManifest)?;
        let bundle = ProjectBundle::from_bytes(&bytes)?;
        let mut media = InMemoryStore::new();
        let mut missing_media = BTreeSet::new();

        for reference in referenced_media(&bundle) {
            let Some(bytes) = self.backend.get(&media_key(reference)).await? else {
                missing_media.insert(reference);
                continue;
            };
            let buffer = decode_media(reference, &bytes)?;
            let actual = media.put(&buffer.samples, buffer.sample_rate);
            if actual != reference {
                return Err(ProjectStoreError::MediaHashMismatch {
                    expected: reference,
                    actual,
                });
            }
        }

        Ok(LoadedProject {
            bundle,
            media,
            missing_media,
        })
    }
}

fn referenced_media(bundle: &ProjectBundle) -> BTreeSet<MediaRef> {
    bundle
        .session
        .phrases
        .values()
        .map(|phrase| phrase.media)
        .collect()
}

fn media_key(reference: MediaRef) -> String {
    let mut key = String::with_capacity(MEDIA_PREFIX.len() + 64 + 4);
    key.push_str(MEDIA_PREFIX);
    for byte in reference.0 {
        use std::fmt::Write as _;
        write!(&mut key, "{byte:02x}").expect("writing to a string cannot fail");
    }
    key.push_str(".wav");
    key
}

/// Encode one mono phrase as a 32-bit float WAV. The audio is stored as an
/// ordinary WAV so a person can extract and import it without Hocket; identity
/// still travels in the content-addressed file name, not the bytes.
fn encode_media(buffer: &MediaBuffer) -> Result<Vec<u8>, ProjectStoreError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: buffer.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec)
        .map_err(|error| ProjectStoreError::MediaEncode(error.to_string()))?;
    for sample in &buffer.samples {
        writer
            .write_sample(*sample)
            .map_err(|error| ProjectStoreError::MediaEncode(error.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|error| ProjectStoreError::MediaEncode(error.to_string()))?;
    Ok(cursor.into_inner())
}

/// Decode a stored WAV phrase back to samples. The caller re-hashes the result
/// against its [`MediaRef`], so a codec or corruption mismatch is caught there.
fn decode_media(reference: MediaRef, bytes: &[u8]) -> Result<MediaBuffer, ProjectStoreError> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).map_err(|_| {
        ProjectStoreError::InvalidMedia {
            reference,
            reason: "unreadable WAV",
        }
    })?;
    let spec = reader.spec();
    if spec.channels != 1 {
        return Err(ProjectStoreError::InvalidMedia {
            reference,
            reason: "expected mono audio",
        });
    }
    if spec.sample_format != hound::SampleFormat::Float || spec.bits_per_sample != 32 {
        return Err(ProjectStoreError::InvalidMedia {
            reference,
            reason: "expected 32-bit float samples",
        });
    }
    let sample_rate = spec.sample_rate;
    let samples = reader
        .samples::<f32>()
        .collect::<Result<Vec<f32>, _>>()
        .map_err(|_| ProjectStoreError::InvalidMedia {
            reference,
            reason: "corrupt sample data",
        })?;
    Ok(MediaBuffer {
        samples,
        sample_rate,
    })
}

#[cfg(test)]
mod tests {
    use muniment::{Backend, MemoryBackend};
    use pollster::block_on;
    use hocket_model::{Edit, History, Layer, Phrase, Session};

    use super::*;

    fn bundle_with_one_layer(store: &mut InMemoryStore) -> ProjectBundle {
        let mut session = Session::new_default();
        let mut history = History::new();
        let media = store.put(&[0.25, -0.5, 0.75], 48_000);
        let phrase = Phrase::new(media, session.bars_per_phrase, session.bpm, 1);
        let layer = Layer::new(phrase.id);
        let track_id = session.tracks[0].id;
        history.commit(
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            },
            &mut session,
            1,
        );
        ProjectBundle::new(session, history)
    }

    #[test]
    fn save_and_load_round_trip_manifest_and_media() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);

            project.save(&bundle, &media).await.unwrap();
            let loaded = project.load().await.unwrap();

            assert_eq!(loaded.bundle, bundle);
            assert!(loaded.missing_media.is_empty());
            let reference = loaded.bundle.session.phrases.values().next().unwrap().media;
            assert_eq!(
                loaded.media.get(&reference).unwrap().samples,
                vec![0.25, -0.5, 0.75]
            );
            assert_eq!(backend.get(MANIFEST_KEY).await.unwrap().is_some(), true);
        });
    }

    /// The archive entries are human-meaningful file names, not opaque keys, so
    /// a saved `.hock` is inspectable. Locks the no-lock-in layout against drift.
    #[test]
    fn save_uses_human_friendly_entry_names() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);

            project.save(&bundle, &media).await.unwrap();

            let mut keys = backend.list("").await.unwrap();
            keys.sort();
            assert_eq!(keys.len(), 2, "one manifest plus one media entry");
            assert_eq!(keys[0], "manifest.cbor");
            assert!(
                keys[1].starts_with("media/") && keys[1].ends_with(".wav"),
                "media entry should be media/<hash>.wav, got {}",
                keys[1]
            );

            // The stored media entry is a real WAV a person could extract.
            let wav = backend.get(&keys[1]).await.unwrap().unwrap();
            assert_eq!(&wav[..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
        });
    }

    #[test]
    fn save_rejects_manifest_that_references_missing_media() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend);
            let mut populated = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut populated);
            let empty = InMemoryStore::new();

            let error = project.save(&bundle, &empty).await.unwrap_err();
            assert!(matches!(error, ProjectStoreError::MissingMedia(_)));
        });
    }

    #[test]
    fn load_keeps_project_when_a_media_blob_is_missing() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);
            backend
                .put(MANIFEST_KEY, &bundle.to_bytes().unwrap())
                .await
                .unwrap();

            let loaded = project.load().await.unwrap();
            assert_eq!(loaded.bundle, bundle);
            assert_eq!(loaded.missing_media.len(), 1);
            assert!(loaded.media.is_empty());
        });
    }

    #[test]
    fn corrupt_media_is_rejected() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);
            let reference = bundle.session.phrases.values().next().unwrap().media;
            backend
                .put(MANIFEST_KEY, &bundle.to_bytes().unwrap())
                .await
                .unwrap();
            backend
                .put(&media_key(reference), b"not audio")
                .await
                .unwrap();

            let error = project.load().await.unwrap_err();
            assert!(matches!(error, ProjectStoreError::InvalidMedia { .. }));
        });
    }

    #[test]
    fn save_rejects_a_corrupt_existing_media_blob() {
        block_on(async {
            let backend = MemoryBackend::new();
            let project = ProjectStore::new(backend.clone());
            let mut media = InMemoryStore::new();
            let bundle = bundle_with_one_layer(&mut media);
            let reference = bundle.session.phrases.values().next().unwrap().media;
            backend
                .put(&media_key(reference), b"not audio")
                .await
                .unwrap();

            let error = project.save(&bundle, &media).await.unwrap_err();
            assert!(matches!(error, ProjectStoreError::InvalidMedia { .. }));
        });
    }
}
