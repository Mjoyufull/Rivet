use matrix_sdk_ui::eyeball_im::Vector;
use std::ops::Range;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PerfLogMode {
    SlowOnly,
    All,
}

pub fn log_if_slow(
    name: &'static str,
    started: Instant,
    threshold: Duration,
    detail: impl FnOnce() -> String,
) {
    let elapsed = started.elapsed();
    if perf_log_mode() == PerfLogMode::SlowOnly && elapsed < threshold {
        return;
    }

    tracing::info!(
        target: "rivet_perf",
        op = name,
        elapsed_ms = elapsed.as_secs_f64() * 1000.0,
        threshold_ms = threshold.as_secs_f64() * 1000.0,
        detail = %detail(),
        "perf"
    );
}

#[doc(hidden)]
pub fn changed_identity_ranges<T>(
    previous_items: &Vector<Arc<T>>,
    current_items: &Vector<Arc<T>>,
) -> (Range<usize>, Range<usize>) {
    let previous_len = previous_items.len();
    let current_len = current_items.len();
    let common_prefix = common_prefix_len(previous_items, current_items);
    let common_suffix = common_suffix_len(previous_items, current_items, common_prefix);

    (
        common_prefix..previous_len.saturating_sub(common_suffix),
        common_prefix..current_len.saturating_sub(common_suffix),
    )
}

#[doc(hidden)]
pub fn expand_incremental_range(counts: &[usize], start: usize, end: usize) -> Range<usize> {
    let len = counts.len();
    if start >= len {
        return len..len;
    }

    let mut expanded_start = start;
    while expanded_start > 0 {
        expanded_start -= 1;
        if counts[expanded_start] > 0 {
            break;
        }
    }

    let mut expanded_end = end.min(len);
    while expanded_end < len {
        expanded_end += 1;
        if counts[expanded_end - 1] > 0 {
            break;
        }
    }

    expanded_start..expanded_end
}

#[doc(hidden)]
pub fn prepare_avatar_bytes_for_bench(input: &[u8]) -> Option<Vec<u8>> {
    crate::models::image_cache::prepare_avatar_bytes_for_bench(input)
}

fn perf_log_mode() -> PerfLogMode {
    static PERF_LOG_MODE: OnceLock<PerfLogMode> = OnceLock::new();
    *PERF_LOG_MODE.get_or_init(|| match std::env::var("RIVET_PERF") {
        Ok(value) if matches!(value.as_str(), "1" | "all" | "trace") => PerfLogMode::All,
        _ => PerfLogMode::SlowOnly,
    })
}

fn common_prefix_len<T>(previous_items: &Vector<Arc<T>>, current_items: &Vector<Arc<T>>) -> usize {
    let max = previous_items.len().min(current_items.len());
    let mut ix = 0;
    while ix < max {
        let Some(previous) = previous_items.get(ix) else {
            break;
        };
        let Some(current) = current_items.get(ix) else {
            break;
        };
        if !Arc::ptr_eq(previous, current) {
            break;
        }
        ix += 1;
    }
    ix
}

fn common_suffix_len<T>(
    previous_items: &Vector<Arc<T>>,
    current_items: &Vector<Arc<T>>,
    common_prefix: usize,
) -> usize {
    let mut previous_ix = previous_items.len();
    let mut current_ix = current_items.len();
    let mut suffix = 0;

    while previous_ix > common_prefix && current_ix > common_prefix {
        let Some(previous) = previous_items.get(previous_ix - 1) else {
            break;
        };
        let Some(current) = current_items.get(current_ix - 1) else {
            break;
        };
        if !Arc::ptr_eq(previous, current) {
            break;
        }
        previous_ix -= 1;
        current_ix -= 1;
        suffix += 1;
    }

    suffix
}
