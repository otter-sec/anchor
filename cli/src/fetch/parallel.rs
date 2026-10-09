use {super::FetchTuning, std::thread};

const DEFAULT_PARALLEL_FETCH_SIGNATURE_THRESHOLD: usize = 10;
const DEFAULT_MAX_PARALLEL_FETCH_WORKERS: usize = 4;

// Decides whether historical transaction fetches should fan out across worker threads.
pub(super) fn should_parallelize_historical_fetch(
    signature_count: usize,
    tuning: &FetchTuning,
) -> bool {
    if tuning.no_parallel {
        return false;
    }
    if matches!(tuning.workers, Some(1)) {
        return false;
    }
    if tuning.workers.is_some_and(|workers| workers > 1) {
        return signature_count > 1;
    }
    signature_count > DEFAULT_PARALLEL_FETCH_SIGNATURE_THRESHOLD
}

// Chooses the worker count for parallel fetches, capped by runtime and CLI limits.
pub(super) fn historical_fetch_worker_count(signature_count: usize, tuning: &FetchTuning) -> usize {
    if !should_parallelize_historical_fetch(signature_count, tuning) {
        return 1;
    }

    let available = thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(DEFAULT_MAX_PARALLEL_FETCH_WORKERS);
    let cap = tuning.workers.unwrap_or(DEFAULT_MAX_PARALLEL_FETCH_WORKERS);

    signature_count.min(available).min(cap).max(1)
}
