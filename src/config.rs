use clap::Parser;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use toml;
use tracing::{info, warn};

/// Default database filename
pub const DEFAULT_DATABASE_FILENAME: &str = "srs_server.db";
/// Default backup interval in minutes
pub const DEFAULT_BACKUP_INTERVAL_MINUTES: u64 = 20;
/// Default number of backups to keep
pub const DEFAULT_BACKUP_COUNT: u32 = 10;

/// Configuration for the Hippocampus application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
	/// URL for the database connection
	pub database_url: String,
	/// Duration between periodic backups in minutes
	pub backup_interval_minutes: u64,
	/// Number of periodic backups to keep
	pub backup_count: u32,
	/// Resolved config directory path, if any
	///
	/// It would be an error to read `config_dir` from a config file,
	/// since it could read a path different to the config file's path,
	/// and leave the `Config` in the server containing a different
	/// `config_dir` than it in fact acquired its config from.
	#[serde(skip)]
	pub config_dir: Option<PathBuf>,
	/// Resolved data directory path, if any
	///
	/// This *is* supported in the config file, since it is not inconsistent
	/// to set the `data_dir` inside the config file.
	pub data_dir: Option<PathBuf>,
	/// Resolved state directory path, if any
	///
	/// This *is* supported in the config file, since it is not inconsistent
	/// to set the `state_dir` inside the config file.
	pub state_dir: Option<PathBuf>,
}

/// Builder for [`Config`] with all fields optional.
///
/// Multiple builders can be merged with [`ConfigBuilder::merge`], where
/// the argument's `Some` values take precedence. Call [`ConfigBuilder::build`]
/// to resolve directory paths and apply defaults, producing a final [`Config`].
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigBuilder {
	/// Optional database URL
	#[serde(default)]
	pub database_url: Option<String>,
	/// Optional backup interval (in minutes)
	#[serde(default)]
	pub backup_interval_minutes: Option<u64>,
	/// Optional backup count
	#[serde(default)]
	pub backup_count: Option<u32>,
	/// Optional server URL for the CLI to connect to
	#[serde(default)]
	pub server_url: Option<String>,
	/// Optional config directory path
	///
	/// It would be an error to read `config_dir` from a config file,
	/// since it could read a path different to the config file's path,
	/// and leave the `Config` in the server containing a different
	/// `config_dir` than it in fact acquired its config from.
	#[serde(skip)]
	pub config_dir: Option<PathBuf>,
	/// Optional data directory path
	///
	/// This *is* supported in the config file, since it is not inconsistent
	/// to set the `data_dir` inside the config file.
	pub data_dir: Option<PathBuf>,
	/// Optional state directory path
	///
	/// This *is* supported in the config file, since it is not inconsistent
	/// to set the `state_dir` inside the config file.
	pub state_dir: Option<PathBuf>,
}

/// Command line arguments for the application
#[derive(Parser, Debug)]
#[clap(name = "hippocampus", about = "A Spaced Repetition System")]
pub struct CliArgs {
	/// Database URL
	#[clap(long, env = "DATABASE_URL")]
	pub database_url: Option<String>,

	/// Backup interval in minutes
	#[clap(long, env = "BACKUP_INTERVAL_MINUTES")]
	pub backup_interval_minutes: Option<u64>,

	/// Number of backups to keep
	#[clap(long, env = "BACKUP_COUNT")]
	pub backup_count: Option<u32>,

	/// Debug mode
	#[clap(long, env = "HIPPOCAMPUS_DEBUG", default_value_t = false)]
	pub debug: bool,

	/// Override path to the config directory
	#[clap(long, env = "HIPPOCAMPUS_CONFIG_DIR")]
	pub config_dir: Option<PathBuf>,

	/// Override path to the data directory
	#[clap(long, env = "HIPPOCAMPUS_DATA_DIR")]
	pub data_dir: Option<PathBuf>,

	/// Override path to the state directory
	#[clap(long, env = "HIPPOCAMPUS_STATE_DIR")]
	pub state_dir: Option<PathBuf>,

	/// Allow path overrides in debug builds (debug-only flag)
	#[cfg(debug_assertions)]
	#[clap(long, default_value_t = false)]
	pub debug_allow_path_override: bool,
}

impl Config {
	/// Returns the backup interval as a Duration
	pub fn backup_interval(&self) -> Duration {
		Duration::from_secs(self.backup_interval_minutes * 60)
	}
}

impl ConfigBuilder {
	/// Merges `other` into `self`, with `other`'s `Some` values taking precedence.
	pub fn merge(self, other: ConfigBuilder) -> ConfigBuilder {
		ConfigBuilder {
			database_url: other.database_url.or(self.database_url),
			backup_interval_minutes: other
				.backup_interval_minutes
				.or(self.backup_interval_minutes),
			backup_count: other.backup_count.or(self.backup_count),
			server_url: other.server_url.or(self.server_url),
			config_dir: other.config_dir.or(self.config_dir),
			data_dir: other.data_dir.or(self.data_dir),
			state_dir: other.state_dir.or(self.state_dir),
		}
	}

	/// Builds the final [`Config`], resolving directory paths and applying defaults.
	///
	/// `data_dir` and `state_dir` are resolved via [`get_data_dir_path`] and
	/// [`get_state_dir_path`], which may create the directories if they don't exist.
	/// `config_dir` is used as-is (it should be pre-resolved by the caller, since
	/// it's needed to locate the config file before building).
	///
	/// If no `database_url` was explicitly provided, it is derived from the
	/// resolved `data_dir` (or falls back to [`DEFAULT_DATABASE_FILENAME`] in the
	/// current directory).
	pub fn build(self) -> Config {
		let data_dir = get_data_dir_path(self.data_dir);
		let state_dir = get_state_dir_path(self.state_dir);

		let database_url = self.database_url.unwrap_or_else(|| {
			data_dir
				.as_ref()
				.map_or(DEFAULT_DATABASE_FILENAME.to_string(), |p| {
					p.join(DEFAULT_DATABASE_FILENAME)
						.to_string_lossy()
						.to_string()
				})
		});

		Config {
			database_url,
			backup_interval_minutes: self
				.backup_interval_minutes
				.unwrap_or(DEFAULT_BACKUP_INTERVAL_MINUTES),
			backup_count: self.backup_count.unwrap_or(DEFAULT_BACKUP_COUNT),
			config_dir: self.config_dir,
			data_dir,
			state_dir,
		}
	}
}

/// Loads configuration from a TOML file
pub fn config_from_file(config_path: Option<PathBuf>) -> Result<ConfigBuilder, String> {
	// if the config path is None, return the default config
	if config_path.is_none() {
		return Ok(ConfigBuilder::default());
	}

	let config_path = config_path.unwrap();

	if !config_path.exists() {
		info!("Config file not found at {:?}, using defaults", config_path);
		return Ok(ConfigBuilder::default());
	}

	match fs::read_to_string(&config_path) {
		Ok(content) => match toml::from_str::<ConfigBuilder>(&content) {
			Ok(config) => {
				info!("Loaded configuration from {:?}", config_path);
				Ok(config)
			}
			Err(e) => {
				warn!("Failed to parse config file: {}", e);
				Err(format!("Failed to parse config file: {}", e))
			}
		},
		Err(e) => {
			warn!("Failed to read config file: {}", e);
			Err(format!("Failed to read config file: {}", e))
		}
	}
}

/// Loads configuration from command line arguments
pub fn config_from_args(args: CliArgs) -> ConfigBuilder {
	ConfigBuilder {
		database_url: args.database_url,
		backup_interval_minutes: args.backup_interval_minutes,
		backup_count: args.backup_count,
		server_url: None,
		config_dir: args.config_dir,
		data_dir: args.data_dir,
		state_dir: args.state_dir,
	}
}

/// Gets the config directory path
///
/// This function returns the path to the config directory for the application
/// based on the XDG base directory specification.
///
/// Unlike [`get_data_dir_path`] and [`get_state_dir_path`], this function does
/// **not** create the directory if it doesn't exist. If you're pointing at a
/// config directory, it should already contain a config file — there's nothing
/// useful to do with an empty one.
///
/// If the debug flag is set, the function will return None, so that we don't
/// mess with any actual program data during development.
pub fn get_config_dir_path(override_path: Option<PathBuf>) -> Option<PathBuf> {
	if cfg!(debug_assertions) && override_path.is_none() {
		info!("Debug build detected, skipping config file");
		return None;
	}

	if let Some(path) = override_path {
		if !path.exists() {
			info!(
				"Override config path not found at {:?}, using defaults",
				path
			);
			return None;
		}
		return Some(path);
	}

	let mut config_path = match ProjectDirs::from("com", "hippocampus", "hippocampus") {
		Some(proj_dirs) => {
			let config_dir = proj_dirs.config_dir();
			let path = PathBuf::from(config_dir);
			Some(path)
		}
		None => {
			warn!("Could not determine XDG config directory, skipping config file");
			None
		}
	};

	config_path = config_path.and_then(|path| {
		if !path.exists() {
			info!("Config path not found at {:?}, using defaults", path);
			None
		} else {
			Some(path)
		}
	});

	config_path
}

/// Gets the data directory path
///
/// This function returns the path to the data directory for the application
/// based on the XDG base directory specification.
///
/// This function **creates** the directory if it doesn't exist, since data
/// files (e.g. the database) will be written there at runtime.
///
/// If the debug flag is set, the function will return None, so that we don't
/// mess with any actual program data during development.
pub fn get_data_dir_path(override_path: Option<PathBuf>) -> Option<PathBuf> {
	if cfg!(debug_assertions) && override_path.is_none() {
		info!("Debug build detected, skipping data files");
		return None;
	}

	if let Some(path) = override_path {
		if !path.exists() {
			if let Err(e) = fs::create_dir_all(&path) {
				warn!("Failed to create override data directory {:?}: {}", path, e);
				return None;
			}
		}
		return Some(path);
	}

	let data_path = match ProjectDirs::from("com", "hippocampus", "hippocampus") {
		Some(proj_dirs) => {
			let data_dir = proj_dirs.data_dir();
			let path = PathBuf::from(data_dir);
			if !path.exists() {
				if let Err(e) = fs::create_dir_all(&path) {
					warn!("Failed to create XDG data directory {:?}: {}", path, e);
					return None;
				}
			}
			Some(path)
		}
		None => {
			warn!("Could not determine XDG data directory, skipping data files");
			None
		}
	};

	data_path
}

/// Gets the state directory path
///
/// This function returns the path to the state directory for the application
/// based on the XDG base directory specification.
///
/// This function **creates** the directory if it doesn't exist, since state
/// files (e.g. logs) will be written there at runtime.
///
/// If the debug flag is set, the function will return None, so that we don't
/// mess with any actual program data during development.
pub fn get_state_dir_path(override_path: Option<PathBuf>) -> Option<PathBuf> {
	if cfg!(debug_assertions) && override_path.is_none() {
		info!("Debug build detected, skipping state files");
		return None;
	}

	if let Some(path) = override_path {
		if !path.exists() {
			if let Err(e) = fs::create_dir_all(&path) {
				warn!(
					"Failed to create override state directory {:?}: {}",
					path, e
				);
				return None;
			}
		}
		return Some(path);
	}

	let state_path = match ProjectDirs::from("com", "hippocampus", "hippocampus") {
		Some(proj_dirs) => {
			let state_dir = proj_dirs
				.state_dir()
				.map(PathBuf::from)
				.or_else(|| get_data_dir_path(None));
			let Some(state_dir) = state_dir else {
				warn!(
					"Could not determine XDG state directory or data directory, skipping state files"
				);
				return None;
			};

			if !state_dir.exists() {
				if let Err(e) = fs::create_dir_all(&state_dir) {
					warn!(
						"Failed to create XDG state directory {:?}: {}",
						state_dir, e
					);
					return None;
				}
			}
			Some(state_dir)
		}
		None => {
			warn!("Could not determine XDG state directory, skipping state files");
			None
		}
	};

	state_path
}

/// Gets the complete configuration by combining defaults with
/// values from config file, environment variables, and command line arguments
/// in order of increasing precedence
pub fn get_config(args: CliArgs) -> Result<Config, String> {
	#[cfg(debug_assertions)]
	{
		let has_override =
			args.config_dir.is_some() || args.data_dir.is_some() || args.state_dir.is_some();
		if has_override && !args.debug_allow_path_override {
			return Err(
				"Path overrides (--config-dir, --data-dir, --state-dir) are not allowed in \
				 debug builds. Use --debug-allow-path-override to enable them."
					.to_string(),
			);
		}
	}

	// Resolve config dir early — we need it to locate the config file
	let config_dir_path = get_config_dir_path(args.config_dir.clone());

	// Merge file config with args config (args take precedence),
	// then override config_dir with the resolved path.
	let mut builder = config_from_file(config_dir_path.as_ref().map(|p| p.join("config.toml")))?
		.merge(config_from_args(args));
	builder.config_dir = config_dir_path;

	let config = builder.build();

	info!(
		"Final configuration: database_url={}, backup_interval={}min, backup_count={}",
		config.database_url, config.backup_interval_minutes, config.backup_count
	);

	Ok(config)
}

#[cfg(test)]
mod prop_tests;
#[cfg(test)]
mod tests;
