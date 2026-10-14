//! Asynchronous HTTP fetcher worker pool for slippy map tiles.

use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{bounded, Receiver, Sender};
use image::RgbaImage;
use thiserror::Error;

use crate::coord::TileCoordinate;
use crate::source::ImagerySource;

/// Standard User-Agent compliant with OpenStreetMap Tile Usage Policy.
pub const DEFAULT_USER_AGENT: &str = "ROSM/0.1.0 (+https://github.com/tau2c/ROSM)";

/// Default timeout for tile download requests.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Number of background worker threads for downloading tiles.
pub const DEFAULT_WORKER_THREADS: usize = 3;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("HTTP request error: {0}")]
    Http(String),
    #[error("Failed to decode image data: {0}")]
    Decode(String),
}

/// A request to download and decode a single map tile.
#[derive(Debug, Clone)]
pub struct TileFetchRequest {
    pub source: ImagerySource,
    pub coord: TileCoordinate,
}

/// A completed tile fetch result.
pub struct TileFetchResponse {
    pub source_id: &'static str,
    pub coord: TileCoordinate,
    pub result: Result<(Vec<u8>, RgbaImage), FetchError>,
}

/// Worker pool that asynchronously downloads and decodes map tiles in the background.
pub struct TileFetcher {
    request_tx: Sender<TileFetchRequest>,
    response_rx: Receiver<TileFetchResponse>,
    shutdown: Arc<AtomicBool>,
    worker_handles: Vec<thread::JoinHandle<()>>,
}

impl TileFetcher {
    /// Creates a new `TileFetcher` with the default number of worker threads.
    pub fn new() -> Self {
        Self::with_worker_count(DEFAULT_WORKER_THREADS)
    }

    /// Creates a new `TileFetcher` with a custom number of worker threads.
    pub fn with_worker_count(worker_count: usize) -> Self {
        let (request_tx, request_rx) = bounded::<TileFetchRequest>(256);
        let (response_tx, response_rx) = bounded::<TileFetchResponse>(256);
        let shutdown = Arc::new(AtomicBool::new(false));

        let mut worker_handles = Vec::with_capacity(worker_count);

        for worker_id in 0..worker_count {
            let rx = request_rx.clone();
            let tx = response_tx.clone();
            let shutdown_flag = Arc::clone(&shutdown);

            let handle = thread::Builder::new()
                .name(format!("rosm-tile-fetcher-{worker_id}"))
                .spawn(move || {
                    let agent = ureq::AgentBuilder::new()
                        .timeout_connect(DEFAULT_TIMEOUT)
                        .timeout_read(DEFAULT_TIMEOUT)
                        .user_agent(DEFAULT_USER_AGENT)
                        .build();

                    while !shutdown_flag.load(Ordering::Relaxed) {
                        let request = match rx.recv_timeout(Duration::from_millis(200)) {
                            Ok(req) => req,
                            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                        };

                        if shutdown_flag.load(Ordering::Relaxed) {
                            break;
                        }

                        let result =
                            download_and_decode_tile(&agent, &request.source, request.coord);

                        let response = TileFetchResponse {
                            source_id: request.source.id,
                            coord: request.coord,
                            result,
                        };

                        if tx.send(response).is_err() {
                            break;
                        }
                    }
                })
                .expect("failed to spawn tile fetcher worker thread");

            worker_handles.push(handle);
        }

        Self {
            request_tx,
            response_rx,
            shutdown,
            worker_handles,
        }
    }

    /// Submits a tile request to the background download queue.
    ///
    /// Returns `true` if queued, or `false` if the channel is full.
    pub fn request_tile(&self, source: ImagerySource, coord: TileCoordinate) -> bool {
        self.request_tx
            .try_send(TileFetchRequest { source, coord })
            .is_ok()
    }

    /// Tries to receive the next completed tile response without blocking.
    pub fn try_recv_response(&self) -> Option<TileFetchResponse> {
        self.response_rx.try_recv().ok()
    }
}

impl Default for TileFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TileFetcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        for handle in self.worker_handles.drain(..) {
            let _ = handle.join();
        }
    }
}

fn download_and_decode_tile(
    agent: &ureq::Agent,
    source: &ImagerySource,
    coord: TileCoordinate,
) -> Result<(Vec<u8>, RgbaImage), FetchError> {
    let url = coord.format_url(source.url_template);

    let response = match agent.get(&url).call() {
        Ok(res) => res,
        Err(err) => return Err(FetchError::Http(err.to_string())),
    };

    let mut bytes = Vec::new();
    if let Err(err) = response.into_reader().read_to_end(&mut bytes) {
        return Err(FetchError::Http(err.to_string()));
    }

    let dyn_img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(err) => return Err(FetchError::Decode(err.to_string())),
    };

    Ok((bytes, dyn_img.to_rgba8()))
}
