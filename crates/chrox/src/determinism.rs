//! Stable seed derivation for reproducible CLI image and palette pipelines.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use chromoxide_image::ImagePipelineConfig;

use crate::config::{Config, ReproducibilityMode};
use crate::palette::registry::PaletteRecordRef;
use crate::solve_config::PartialSolveConfig;

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const DETERMINISM_VERSION: &[u8] = b"chrox-determinism-v1";
const PALETTE_SOLVE_DOMAIN: &[u8] = b"palette-solve";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedMasterSeed {
    pub seed: u64,
    pub is_random: bool,
}

#[derive(Clone, Copy, Debug)]
struct StableHasher64 {
    state: u64,
}

impl StableHasher64 {
    fn new() -> Self {
        Self { state: FNV_OFFSET }
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= u64::from(byte);
            self.state = self.state.wrapping_mul(FNV_PRIME);
        }
    }

    fn write_component(&mut self, bytes: &[u8]) {
        self.write_raw(&(bytes.len() as u64).to_le_bytes());
        self.write_raw(bytes);
    }

    fn finish(self) -> u64 {
        self.state
    }
}

/// Derives a deterministic 32-byte sub-seed from a master seed and domain.
pub fn derive_seed(master: u64, domain: &[u8], components: &[&[u8]]) -> [u8; 32] {
    let mut hasher = StableHasher64::new();
    hasher.write_component(DETERMINISM_VERSION);
    hasher.write_component(&master.to_le_bytes());
    hasher.write_component(domain);
    for component in components {
        hasher.write_component(component);
    }

    let mut state = hasher.finish();
    let mut output = [0_u8; 32];
    for chunk in output.chunks_exact_mut(std::mem::size_of::<u64>()) {
        chunk.copy_from_slice(&splitmix64_next(&mut state).to_le_bytes());
    }
    output
}

/// Hashes image bytes and canonical image/global solve config into a master seed.
pub fn content_derived_master_seed(
    image_path: &Path,
    image_config: &ImagePipelineConfig,
    global_config: &PartialSolveConfig,
) -> Result<u64, Error> {
    let image_config = toml::to_string(image_config).map_err(|source| Error::Serialize {
        component: "image config",
        source,
    })?;
    let global_config = toml::to_string(global_config).map_err(|source| Error::Serialize {
        component: "global solve config",
        source,
    })?;

    let mut hasher = StableHasher64::new();
    hasher.write_component(DETERMINISM_VERSION);
    write_file_component(&mut hasher, image_path)?;
    hasher.write_component(image_config.as_bytes());
    hasher.write_component(global_config.as_bytes());
    Ok(hasher.finish())
}

/// Resolves the effective CLI master seed.
///
/// Precedence is explicit seed, randomize flag, then configuration.
pub fn resolve_master_seed(
    image_path: &Path,
    config: &Config,
    explicit_seed: Option<u64>,
    randomize: bool,
) -> Result<ResolvedMasterSeed, Error> {
    config.validate().map_err(Error::Config)?;

    if let Some(seed) = explicit_seed {
        return Ok(ResolvedMasterSeed {
            seed,
            is_random: false,
        });
    }
    if randomize {
        return Ok(random_master_seed());
    }

    match (
        config.general.reproducibility.mode,
        config.general.reproducibility.seed,
    ) {
        (ReproducibilityMode::ContentDerived, None) => Ok(ResolvedMasterSeed {
            seed: content_derived_master_seed(image_path, &config.image, &config.config)?,
            is_random: false,
        }),
        (ReproducibilityMode::Fixed, Some(seed)) => Ok(ResolvedMasterSeed {
            seed,
            is_random: false,
        }),
        (ReproducibilityMode::Random, None) => Ok(random_master_seed()),
        _ => unreachable!("reproducibility configuration was validated"),
    }
}

/// Derives a palette-specific solver seed without consuming shared RNG state.
pub fn palette_seed(
    master: u64,
    palette: PaletteRecordRef<'_>,
) -> Result<chromoxide::SolveSeed, Error> {
    let palette_id = match palette {
        PaletteRecordRef::User(record) => record.id.as_bytes(),
        PaletteRecordRef::Builtin(record) => record.id.as_bytes(),
    };
    let fingerprint = palette_fingerprint(palette)?;
    Ok(derive_seed(
        master,
        PALETTE_SOLVE_DOMAIN,
        &[palette_id, &fingerprint],
    ))
}

fn random_master_seed() -> ResolvedMasterSeed {
    ResolvedMasterSeed {
        seed: rand::random(),
        is_random: true,
    }
}

fn palette_fingerprint(palette: PaletteRecordRef<'_>) -> Result<[u8; 8], Error> {
    let mut hasher = StableHasher64::new();
    match palette {
        PaletteRecordRef::Builtin(record) => {
            hasher.write_component(b"builtin");
            hasher.write_component(record.id.as_bytes());
        }
        PaletteRecordRef::User(record) => {
            hasher.write_component(b"user");
            hasher.write_component(record.id.as_bytes());
            match record.palette.source_path.as_deref() {
                Some(path) => write_file_component(&mut hasher, path)?,
                None => {
                    let serialized =
                        toml::to_string(&record.palette).map_err(|source| Error::Serialize {
                            component: "user palette",
                            source,
                        })?;
                    hasher.write_component(serialized.as_bytes());
                }
            }
        }
    }
    Ok(hasher.finish().to_le_bytes())
}

fn write_file_component(hasher: &mut StableHasher64, path: &Path) -> Result<(), Error> {
    let file = File::open(path).map_err(|source| Error::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let expected_len = file
        .metadata()
        .map_err(|source| Error::ReadFile {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    hasher.write_raw(&expected_len.to_le_bytes());

    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 16 * 1024];
    let mut actual_len = 0_u64;
    loop {
        let read = reader.read(&mut buffer).map_err(|source| Error::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        actual_len = actual_len.saturating_add(read as u64);
        hasher.write_raw(&buffer[..read]);
    }

    if actual_len != expected_len {
        return Err(Error::FileLengthChanged {
            path: path.to_path_buf(),
            expected_len,
            actual_len,
        });
    }
    Ok(())
}

fn splitmix64_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read deterministic seed input `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "deterministic seed input `{path}` changed length while being read (expected {expected_len}, read {actual_len})"
    )]
    FileLengthChanged {
        path: PathBuf,
        expected_len: u64,
        actual_len: u64,
    },
    #[error("failed to serialize {component} for deterministic seed derivation")]
    Serialize {
        component: &'static str,
        #[source]
        source: toml::ser::Error,
    },
    #[error("invalid configuration for deterministic seed resolution")]
    Config(#[source] crate::config::Error),
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::config::{Config, ReproducibilityConfig, ReproducibilityMode};
    use crate::palette::registry::{PaletteRecordRef, PaletteRegistry, UserPaletteRecord};
    use crate::palette::user::PaletteFile;

    use super::{
        StableHasher64, content_derived_master_seed, derive_seed, palette_seed, resolve_master_seed,
    };

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "chrox-determinism-{label}-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("test directory should be created");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn hash_components(components: &[&[u8]]) -> u64 {
        let mut hasher = StableHasher64::new();
        for component in components {
            hasher.write_component(component);
        }
        hasher.finish()
    }

    #[test]
    fn stable_hash_respects_component_boundaries() {
        assert_ne!(
            hash_components(&[b"ab", b"c"]),
            hash_components(&[b"a", b"bc"])
        );
    }

    #[test]
    fn stable_hash_matches_golden_vector() {
        assert_eq!(hash_components(&[b"ab", b"c"]), 9_106_356_563_233_852_118);
    }

    #[test]
    fn derived_seed_matches_golden_vector() {
        assert_eq!(
            derive_seed(42, b"image-support", &[]),
            [
                151, 181, 85, 156, 209, 93, 212, 146, 139, 133, 49, 91, 160, 65, 155, 22, 244, 14,
                66, 39, 35, 149, 29, 57, 39, 150, 4, 68, 109, 141, 212, 216,
            ]
        );
    }

    #[test]
    fn content_derived_master_seed_matches_golden_vector() {
        let dir = TestDir::new("golden-master");
        let image = dir.path().join("image.bin");
        std::fs::write(&image, b"golden image bytes").unwrap();
        let config = Config::default();

        assert_eq!(
            content_derived_master_seed(&image, &config.image, &config.config).unwrap(),
            11_703_915_750_292_186_511
        );
    }

    #[test]
    fn same_image_bytes_at_different_paths_have_same_master_seed() {
        let dir = TestDir::new("same-bytes");
        let first = dir.path().join("first.bin");
        let second = dir.path().join("second.bin");
        std::fs::write(&first, b"identical image payload").unwrap();
        std::fs::write(&second, b"identical image payload").unwrap();
        let config = Config::default();

        let first_seed =
            content_derived_master_seed(&first, &config.image, &config.config).unwrap();
        let second_seed =
            content_derived_master_seed(&second, &config.image, &config.config).unwrap();

        assert_eq!(first_seed, second_seed);
    }

    #[test]
    fn image_byte_change_changes_master_seed() {
        let dir = TestDir::new("changed-bytes");
        let first = dir.path().join("first.bin");
        let second = dir.path().join("second.bin");
        std::fs::write(&first, b"image payload A").unwrap();
        std::fs::write(&second, b"image payload B").unwrap();
        let config = Config::default();

        let first_seed =
            content_derived_master_seed(&first, &config.image, &config.config).unwrap();
        let second_seed =
            content_derived_master_seed(&second, &config.image, &config.config).unwrap();

        assert_ne!(first_seed, second_seed);
    }

    #[test]
    fn image_config_change_changes_master_seed() {
        let dir = TestDir::new("image-config");
        let image = dir.path().join("image.bin");
        std::fs::write(&image, b"image payload").unwrap();
        let first = Config::default();
        let mut second = first.clone();
        second.image.preprocess.background_rgb8 = [12, 34, 56];

        let first_seed = content_derived_master_seed(&image, &first.image, &first.config).unwrap();
        let second_seed =
            content_derived_master_seed(&image, &second.image, &second.config).unwrap();

        assert_ne!(first_seed, second_seed);
    }

    #[test]
    fn global_solve_config_change_changes_master_seed() {
        let dir = TestDir::new("solve-config");
        let image = dir.path().join("image.bin");
        std::fs::write(&image, b"image payload").unwrap();
        let first = Config::default();
        let mut second = first.clone();
        second.config.seed_count = Some(31);

        let first_seed = content_derived_master_seed(&image, &first.image, &first.config).unwrap();
        let second_seed =
            content_derived_master_seed(&image, &second.image, &second.config).unwrap();

        assert_ne!(first_seed, second_seed);
    }

    #[test]
    fn image_and_palette_domains_get_different_subseeds() {
        let master = 42;
        let image_seed = derive_seed(master, b"image-support", &[]);
        let palette_seed = derive_seed(master, b"palette-solve", &[b"demo", b"fingerprint"]);

        assert_ne!(image_seed, palette_seed);
    }

    #[test]
    fn fixed_seed_mode_returns_requested_seed() {
        let mut config = Config::default();
        config.general.reproducibility = ReproducibilityConfig {
            mode: ReproducibilityMode::Fixed,
            seed: Some(42),
        };

        let resolved =
            resolve_master_seed(Path::new("missing-image-is-not-read"), &config, None, false)
                .unwrap();

        assert_eq!(resolved.seed, 42);
        assert!(!resolved.is_random);
    }

    #[test]
    fn explicit_seed_override_is_not_random() {
        let mut config = Config::default();
        config.general.reproducibility = ReproducibilityConfig {
            mode: ReproducibilityMode::Random,
            seed: None,
        };

        let resolved = resolve_master_seed(
            Path::new("missing-image-is-not-read"),
            &config,
            Some(73),
            true,
        )
        .unwrap();

        assert_eq!(resolved.seed, 73);
        assert!(!resolved.is_random);
    }

    #[test]
    fn palette_seed_is_independent_of_palette_order() {
        let registry = PaletteRegistry::default();
        let ansi = registry.resolve("ansi-16").unwrap();
        let base16 = registry.resolve("base16").unwrap();

        let ansi_first = palette_seed(99, ansi).unwrap();
        let base16_second = palette_seed(99, base16).unwrap();
        let base16_first = palette_seed(99, base16).unwrap();
        let ansi_second = palette_seed(99, ansi).unwrap();

        assert_eq!(ansi_first, ansi_second);
        assert_eq!(base16_first, base16_second);
    }

    #[test]
    fn different_palette_ids_get_different_subseeds() {
        let registry = PaletteRegistry::default();
        let ansi = registry.resolve("ansi-16").unwrap();
        let base16 = registry.resolve("base16").unwrap();

        assert_ne!(
            palette_seed(99, ansi).unwrap(),
            palette_seed(99, base16).unwrap()
        );
    }

    #[test]
    fn user_palette_fingerprint_uses_source_file_bytes() {
        let dir = TestDir::new("user-source");
        let first_path = dir.path().join("first.toml");
        let second_path = dir.path().join("second.toml");
        std::fs::write(&first_path, "id = \"demo\"\nname = \"Demo\"\n").unwrap();
        std::fs::write(
            &second_path,
            "# changes the source fingerprint\nid = \"demo\"\nname = \"Demo\"\n",
        )
        .unwrap();
        let first = UserPaletteRecord {
            id: "demo".to_string(),
            path: first_path.clone(),
            palette: PaletteFile::from_path(first_path).unwrap(),
        };
        let second = UserPaletteRecord {
            id: "demo".to_string(),
            path: second_path.clone(),
            palette: PaletteFile::from_path(second_path).unwrap(),
        };

        assert_ne!(
            palette_seed(99, PaletteRecordRef::User(&first)).unwrap(),
            palette_seed(99, PaletteRecordRef::User(&second)).unwrap()
        );
    }

    #[test]
    fn in_memory_user_palette_fingerprint_uses_serialized_palette() {
        let first = UserPaletteRecord {
            id: "demo".to_string(),
            path: PathBuf::new(),
            palette: PaletteFile::from_str("id = \"demo\"\nname = \"Demo\"\n").unwrap(),
        };
        let second = UserPaletteRecord {
            id: "demo".to_string(),
            path: PathBuf::new(),
            palette: PaletteFile::from_str(
                "id = \"demo\"\nname = \"Demo\"\n[config]\nseed_count = 24\n",
            )
            .unwrap(),
        };

        assert_ne!(
            palette_seed(99, PaletteRecordRef::User(&first)).unwrap(),
            palette_seed(99, PaletteRecordRef::User(&second)).unwrap()
        );
    }
}
