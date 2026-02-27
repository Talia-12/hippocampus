use std::path::PathBuf;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use clap::Parser;
use std::fs;
use tracing::{info, warn};
use toml;

/// Configuration for the Hippocampus application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// URL for the database connection
    pub database_url: String,
    /// Duration between periodic backups in minutes
    pub backup_interval_minutes: u64,
    /// Number of periodic backups to keep
    pub backup_count: u32,
}

/// Update structure for Config with all fields optional
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigUpdate {
    /// Optional update for database URL
    #[serde(default)]
    pub database_url: Option<String>,
    /// Optional update for backup interval (in minutes)
    #[serde(default)]
    pub backup_interval_minutes: Option<u64>,
    /// Optional update for backup count
    #[serde(default)]
    pub backup_count: Option<u32>,
    /// Optional server URL for the CLI to connect to
    #[serde(default)]
    pub server_url: Option<String>,
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
}

impl Config {
    /// Applies a config update to the current configuration
    pub fn apply_update(self, update: ConfigUpdate) -> Self {
        Self {
            database_url: update.database_url.unwrap_or(self.database_url),
            backup_interval_minutes: update.backup_interval_minutes.unwrap_or(self.backup_interval_minutes),
            backup_count: update.backup_count.unwrap_or(self.backup_count),
        }
    }
    
    /// Returns the backup interval as a Duration
    pub fn backup_interval(&self) -> Duration {
        Duration::from_secs(self.backup_interval_minutes * 60)
    }
}

/// Returns the base (default) configuration
pub fn base_config(data_dir_path: Option<PathBuf>) -> Config {

    let database_url = data_dir_path.map_or("srs_server.db".to_string(), |path| path.join("srs_server.db").to_string_lossy().to_string());

    Config {
        database_url,
        backup_interval_minutes: 20,
        backup_count: 10,
    }
}

/// Loads configuration from a TOML file
pub fn config_from_file(config_path: Option<PathBuf>) -> Result<ConfigUpdate, String> {
    // if the config path is None, return the default config
    if config_path.is_none() {
            return Ok(ConfigUpdate::default());
        }

    let config_path = config_path.unwrap();

    if !config_path.exists() {
        info!("Config file not found at {:?}, using defaults", config_path);
        return Ok(ConfigUpdate::default());
    }

    match fs::read_to_string(&config_path) {
        Ok(content) => match toml::from_str::<ConfigUpdate>(&content) {
            Ok(config) => {
                info!("Loaded configuration from {:?}", config_path);
                Ok(config)
            },
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
pub fn config_from_args(args: CliArgs) -> ConfigUpdate {
    ConfigUpdate {
        database_url: args.database_url,
        backup_interval_minutes: args.backup_interval_minutes,
        backup_count: args.backup_count,
        server_url: None,
    }
}


/// Gets the config directory path
///
/// This function returns the path to the config directory for the application
/// based on the XDG base directory specification.
/// 
/// If the debug flag is set, the function will return None, so that we don't
/// mess with any actual program data during development.
pub fn get_config_dir_path() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        info!("Debug build detected, skipping config file");
        return None;
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
/// If the debug flag is set, the function will return None, so that we don't
/// mess with any actual program data during development.
pub fn get_data_dir_path() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        info!("Debug build detected, skipping state files");
        return None;
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
/// If the debug flag is set, the function will return None, so that we don't
/// mess with any actual program data during development.
pub fn get_state_dir_path() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        info!("Debug build detected, skipping state files");
        return None;
    }

    let state_path = match ProjectDirs::from("com", "hippocampus", "hippocampus") {
        Some(proj_dirs) => {
            let state_dir = proj_dirs
                .state_dir()
                .map(PathBuf::from)
                .or_else(get_data_dir_path);
            let Some(state_dir) = state_dir else {
                warn!("Could not determine XDG state directory or data directory, skipping state files");
                return None;
            };
            
            if !state_dir.exists() {
                if let Err(e) = fs::create_dir_all(&state_dir) {
                    warn!("Failed to create XDG state directory {:?}: {}", state_dir, e);
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
    let config_dir_path = get_config_dir_path();
    let data_dir_path = get_data_dir_path();

    let base = base_config(data_dir_path.clone());
    
    // Apply updates in order of increasing precedence
    let config = base
        .apply_update(config_from_file(config_dir_path.map(|path| path.join("config.toml")))?)
        .apply_update(config_from_args(args));
    
    info!("Final configuration: database_url={}, backup_interval={}min, backup_count={}", 
          config.database_url, config.backup_interval_minutes, config.backup_count);
    
    Ok(config)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod prop_tests;
