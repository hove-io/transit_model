// Copyright (C) 2025 Hove and/or its affiliates.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by the
// Free Software Foundation, version 3.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.

// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>

//! A module for managing downloadable transit models with Navitia integration
//!
//! This system provides:
//! 1. Automatic updates from Navitia-powered transit feeds
//! 2. Secure authentication for Navitia API
//! 3. Thread-safe model access
//! 4. Configurable update intervals
//! 5. Pluggable download implementations

use crate::ntfs;
use reqwest::header;
use serde::Deserialize;
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

/// Configuration for model management
#[derive(Clone, Deserialize)]
pub struct ModelConfig {
    /// Update check interval in seconds
    pub check_interval_secs: u64,
    /// Local path for storing downloaded models
    pub path: String,
}

/// Configuration for Navitia API connection
#[derive(Clone, Deserialize)]
pub struct NavitiaConfig {
    /// Base URL of Navitia API
    pub navitia_url: String,
    /// Coverage area identifier
    pub coverage: String,
    /// Authentication token for Navitia
    pub navitia_token: String,
}

type DownloadResult =
    Pin<Box<dyn Future<Output = Result<String, Box<dyn Error + Send + Sync>>> + Send>>;

/// Trait defining model download functionality
pub trait Downloader: Send + Sync + 'static {
    /// Downloads a specific model version
    ///
    /// # Arguments
    /// * `config` - Model configuration
    /// * `version` - Target version identifier
    ///
    /// # Returns
    /// Future resolving to local path of downloaded model
    fn run_download(&self, config: &ModelConfig, version: &str) -> DownloadResult;
}

/// Main structure managing the transit model lifecycle
pub struct DownloadableTransitModel<D: Downloader> {
    /// Thread-safe model access
    pub current_model: Arc<RwLock<crate::Model>>,
    /// Current model version
    version: Arc<Mutex<String>>,
    /// Download implementation
    downloader: D,
    /// Model configuration
    config: ModelConfig,
    /// Navitia API configuration
    navitia_config: NavitiaConfig,
}

impl<D: Downloader + Clone> DownloadableTransitModel<D> {
    /// Initializes a new model manager
    ///
    /// # Arguments
    /// * `navitia_config` - Navitia API credentials
    /// * `config` - Model management settings
    /// * `downloader` - Download implementation
    ///
    /// # Flow
    /// 1. Fetch current version from Navitia
    /// 2. Download initial model
    /// 3. Start background updater
    pub async fn new(
        navitia_config: NavitiaConfig,
        config: ModelConfig,
        downloader: D,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let (model, version) =
            Self::initialize_model(&config, &navitia_config, &downloader).await?;

        let instance = Self {
            current_model: Arc::new(RwLock::new(model)),
            version: Arc::new(Mutex::new(version)),
            downloader,
            config,
            navitia_config,
        };

        instance.start_background_updater();
        Ok(instance)
    }

    /// Fetches the current model
    ///
    /// # Arguments
    /// * `config` - Model configuration
    /// * `navitia_config` - Navitia API credentials
    /// * `downloader` - Download implementation
    ///
    /// # Returns
    /// A reference to the current model
    /// A reference to the current model version path
    async fn initialize_model(
        config: &ModelConfig,
        navitia_config: &NavitiaConfig,
        downloader: &D,
    ) -> Result<(crate::Model, String), Box<dyn Error + Send + Sync>> {
        let version = Self::get_remote_version(navitia_config).await?;
        let folder_saved_at_path = downloader.run_download(config, &version).await?;
        let model = ntfs::read(&folder_saved_at_path).map_err(anyhow::Error::from)?;
        Ok((model, version))
    }

    /// Starts the background updater
    ///
    /// # Arguments
    ///
    /// * `self` - The current instance
    ///
    /// # Flow
    ///
    /// 1. Periodically checks for updates
    /// 2. Downloads and updates the model if a new version is available
    /// 3. Logs the update
    ///
    /// # Note
    ///
    /// This function runs indefinitely in the background
    ///
    fn start_background_updater(&self) {
        let config = self.config.clone();
        let downloader = self.downloader.clone();
        let model_ref = self.current_model.clone();
        let version_ref = self.version.clone();
        let navitia_config = self.navitia_config.clone();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.check_interval_secs));
            loop {
                interval.tick().await;
                match Self::check_and_update(
                    &config,
                    &downloader,
                    &model_ref,
                    &version_ref,
                    &navitia_config,
                )
                .await
                {
                    Ok(updated) => {
                        if updated {
                            println!("Updated to version {}", *version_ref.lock().await);
                        }
                    }
                    Err(e) => eprintln!("Background update failed: {}", e),
                }
            }
        });
    }

    /// Checks for updates and updates the model if a new version is available
    ///
    /// # Arguments
    ///
    /// * `config` - Model configuration
    /// * `downloader` - Download implementation
    /// * `model` - The current model
    /// * `version` - The current model version
    /// * `navitia_config` - Navitia API credentials
    ///
    /// # Returns
    ///
    /// A boolean indicating whether the model was updated
    async fn check_and_update(
        config: &ModelConfig,
        downloader: &D,
        model: &Arc<RwLock<crate::Model>>,
        version: &Mutex<String>,
        navitia_config: &NavitiaConfig,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let remote_version = Self::get_remote_version(navitia_config).await?;
        let current_version = version.lock().await.clone();

        if remote_version > current_version {
            // Download and load the model before acquiring the write lock
            let saved_to_path = downloader.run_download(config, &remote_version).await?;
            let new_model = ntfs::read(&saved_to_path).map_err(anyhow::Error::from)?;

            let mut model_lock = model.write().await;
            *model_lock = new_model;

            // Update version
            let mut version_lock = version.lock().await;
            *version_lock = remote_version;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Fetches the current version from Navitia
    ///
    /// # Arguments
    ///
    /// * `config` - Navitia API configuration
    ///
    /// # Returns
    ///
    /// The current version identifier
    async fn get_remote_version(
        config: &NavitiaConfig,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        #[derive(Deserialize)]
        struct Status {
            dataset_created_at: String,
        }

        #[derive(Deserialize)]
        struct StatusResponse {
            status: Status,
        }

        let url = format!("{}/coverage/{}/status", config.navitia_url, config.coverage);
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", config.navitia_token),
            )
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to fetch status: {}", response.status()).into());
        }

        let res = response.json::<StatusResponse>().await?;
        Ok(res.status.dataset_created_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::time::{self, Duration};

    #[derive(Clone)]
    struct MockDownloader;

    impl Downloader for MockDownloader {
        fn run_download(&self, _config: &ModelConfig, _version: &str) -> DownloadResult {
            Box::pin(async move {
                time::sleep(Duration::from_secs(1)).await;
                Ok("tests/fixtures/minimal_ntfs/".into())
            })
        }
    }

    fn create_test_config() -> (ModelConfig, NavitiaConfig) {
        (
            ModelConfig {
                check_interval_secs: 1,
                path: "test_path".into(),
            },
            NavitiaConfig {
                navitia_url: mockito::server_url(),
                coverage: "test_coverage".into(),
                navitia_token: "test_token".into(),
            },
        )
    }

    async fn create_mock_navitia_response(version: &str) -> mockito::Mock {
        mock("GET", "/coverage/test_coverage/status")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{"status": {{"dataset_created_at": "{}"}}}}"#,
                version
            ))
            .create()
    }

    #[tokio::test]
    async fn test_initialization() {
        let (config, navitia_config) = create_test_config();
        let _m = create_mock_navitia_response("1.0.0").await;

        let model = DownloadableTransitModel::<MockDownloader>::new(
            navitia_config,
            config,
            MockDownloader {},
        )
        .await
        .unwrap();
        let version = model.version.lock().await;
        assert_eq!(*version, "1.0.0");
    }

    #[tokio::test]
    async fn test_background_update() {
        let (config, navitia_config) = create_test_config();
        let _m = create_mock_navitia_response("1.0.0").await;

        let model = DownloadableTransitModel::<MockDownloader>::new(
            navitia_config.clone(),
            config.clone(),
            MockDownloader {},
        )
        .await
        .unwrap();
        let version = model.version.clone();
        let model_ref = model.current_model.clone();

        // update to 1.0.1 version
        let _m = create_mock_navitia_response("1.0.1").await;

        // Force an update
        let updated = DownloadableTransitModel::<MockDownloader>::check_and_update(
            &config,
            &MockDownloader {},
            &model_ref,
            &version,
            &navitia_config,
        )
        .await
        .unwrap();
        assert!(updated);

        let version = model.version.lock().await;
        assert_eq!(*version, "1.0.1");
    }

    #[tokio::test]
    async fn test_no_update_when_version_same() {
        let _m = create_mock_navitia_response("1.0.0").await;
        let (model_config, navitia_config) = create_test_config();
        let downloader = MockDownloader {};

        let model_manager = DownloadableTransitModel::new(
            navitia_config.clone(),
            model_config.clone(),
            downloader.clone(),
        )
        .await
        .unwrap();

        let updated = DownloadableTransitModel::check_and_update(
            &model_config,
            &downloader,
            &model_manager.current_model,
            &model_manager.version,
            &navitia_config,
        )
        .await
        .unwrap();

        assert!(!updated);
    }

    #[tokio::test]
    async fn test_concurrent_access_during_update() {
        let _m = create_mock_navitia_response("1.0.0").await;
        let (model_config, navitia_config) = create_test_config();
        let downloader = MockDownloader {};

        let model_manager = DownloadableTransitModel::new(navitia_config, model_config, downloader)
            .await
            .unwrap();

        let handle = model_manager.current_model.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();

        // Spawn a reader that holds the lock
        tokio::spawn(async move {
            let _guard = handle.read().await;
            let lines = _guard
                .lines
                .get_idx("M1")
                .iter()
                .map(|idx| _guard.lines[*idx].name.to_string())
                .collect::<Vec<String>>();
            assert_eq!(lines, vec!["Metro 1".to_string(),]);
            tx.send(()).unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        });

        // Wait for read lock acquisition
        rx.await.unwrap();

        // Attempt update
        let update_handle = model_manager.current_model.clone();
        tokio::spawn(async move {
            let _guard = update_handle.write().await;
        });
    }

    #[tokio::test]
    async fn test_atomic_update() {
        let version_counter = Arc::new(AtomicUsize::new(0));
        let (model_config, navitia_config) = create_test_config();

        #[derive(Clone)]
        struct CountingDownloader {
            counter: Arc<AtomicUsize>,
        }

        impl Downloader for CountingDownloader {
            fn run_download(&self, _: &ModelConfig, _: &str) -> DownloadResult {
                let counter = self.counter.clone();
                Box::pin(async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok("tests/fixtures/minimal_ntfs/".into())
                })
            }
        }

        let downloader = CountingDownloader {
            counter: version_counter.clone(),
        };

        let _m = create_mock_navitia_response("1.0.0").await;
        let model_manager = DownloadableTransitModel::new(
            navitia_config.clone(),
            model_config.clone(),
            downloader.clone(),
        )
        .await
        .unwrap();

        // Trigger multiple update checks
        for _ in 0..5 {
            let _ = DownloadableTransitModel::check_and_update(
                &model_config,
                &downloader,
                &model_manager.current_model,
                &model_manager.version,
                &navitia_config,
            )
            .await;
        }

        assert_eq!(version_counter.load(Ordering::SeqCst), 1);
    }
}
