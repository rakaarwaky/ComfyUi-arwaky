#[derive(Clone)]
pub enum DownloadEvent {
    Start {
        worker_id: usize,
        filename: String,
    },
    Progress {
        worker_id: usize,
        filename: String,
        downloaded: u64,
        total: u64,
        speed_mb_s: f64,
        eta_secs: u64,
    },
    ModelFinished {
        worker_id: usize,
        filename: String,
        success: bool,
        error_msg: Option<String>,
    },
    AllComplete {
        completed: usize,
        failed: usize,
    },
    RefreshUpdate {
        idx: usize,
        size: u64,
    },
    RefreshFinished {
        valid: usize,
        invalid: usize,
        unknown: usize,
    },
}
