// SPDX-License-Identifier: MIT OR Apache-2.0

//! Set up logging through a [`tracing`](https://docs.rs/tracing) subscriber.
//!
//! This module configures [`tracing_subscriber`](https://docs.rs/tracing_subscriber) with up to two output layers:
//! - **stdout** – human-friendly, ANSI-coloured when attached to a real TTY.
//! - **file** – plain-text, appended to [`LOG_FILE`] inside `data_dir` via a
//!   non-blocking writer.
//!
//! The active log level is controlled (in descending priority) by:
//! 1. The `RUST_LOG` environment variable.
//! 2. The `--debug` flag (`debug` level) or its absence (`info` level).
//!
//! When the `tokio-console` feature is enabled, the registry also enables
//! `tokio=trace` and `runtime=trace` so that `tokio-console` can connect,
//! while the human-facing layers stay quiet through their own per-layer filters.

use std::fs;
use std::io;
use std::io::IsTerminal;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

/// The file which logging events are written to.
pub(crate) const LOG_FILE: &str = "debug.log";

/// Initialises the global [`tracing`] subscriber for the application.
///
/// Depending on the flags provided, the subscriber can write to stdout, a log
/// file, both, or neither. The two layers are independent and may be enabled in
/// any combination at runtime.
///
/// # Arguments
///
/// * `data_dir` – Directory in which [`LOG_FILE`] is created when
///   `log_to_file` is `true`. The directory must already exist.
/// * `log_to_file` – Append structured log output to `<data_dir>/`[`LOG_FILE`].
/// * `log_to_stdout` – Emit log output to stdout.
/// * `debug` – Set the default log level to `debug`. When `false` the
///   level defaults to `info`. In both cases `RUST_LOG` overrides the default.
///
/// # Returns
///
/// Returns `Ok(Some(guard))` when file logging is active. The [`WorkerGuard`]
/// must be kept alive for the duration of the program; dropping it flushes and
/// shuts down the non-blocking file-writer thread. Returns `Ok(None)` when file
/// logging is disabled.
///
/// # Errors
///
/// Returns [`io::Error`] if `log_to_file` is `true` and [`LOG_FILE`] cannot be
/// created or opened for appending inside `data_dir`.
///
/// # Panics
///
/// Panics if a global [`tracing`] subscriber has already been installed (i.e.
/// this function is called more than once).
pub fn start_logger(
    data_dir: &str,
    log_to_file: bool,
    log_to_stdout: bool,
    debug: bool,
) -> Result<Option<WorkerGuard>, io::Error> {
    // Get the log level from `--debug`.
    let log_level = if debug { "debug" } else { "info" };

    // Try to build an `EnvFilter` from the `RUST_LOG` env variable, or fallback to `log_level`.
    let log_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    // For the registry, also enable very verbose runtime traces so `tokio-console` works, but keep
    // human outputs quiet via per-layer filters below.
    #[cfg(feature = "tokio-console")]
    let base_filter = EnvFilter::new(format!("{log_filter},tokio=trace,runtime=trace"));

    #[cfg(not(feature = "tokio-console"))]
    let base_filter = log_filter.clone();

    // Validate the log file path.
    if log_to_file {
        let file_path = format!("{data_dir}/{LOG_FILE}");
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
    }

    // Timer for log events.
    let log_timer = ChronoLocal::new("%Y-%m-%d %H:%M:%S".to_string());

    // Standard Output layer: human-friendly formatting and level; ANSI only on a real TTY.
    let fmt_layer_stdout = log_to_stdout.then(|| {
        fmt::layer()
            .with_writer(io::stdout)
            .with_ansi(IsTerminal::is_terminal(&io::stdout()))
            .with_timer(log_timer.clone())
            .with_target(true)
            .with_level(true)
            .with_filter(log_filter.clone())
    });

    // File layer: non-blocking writer. Keep the `WorkerGuard` so logs flush on drop.
    let mut guard = None;
    let fmt_layer_logfile = log_to_file.then(|| {
        let file_appender = tracing_appender::rolling::never(data_dir, "debug.log");
        let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);
        guard = Some(file_guard);

        fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false)
            .with_timer(log_timer)
            .with_target(true)
            .with_level(true)
            .with_filter(log_filter.clone())
    });

    // Build the registry with its (possibly more permissive) base filter, then attach layers to it.
    let registry = tracing_subscriber::registry().with(base_filter);

    #[cfg(feature = "tokio-console")]
    let registry = registry.with(console_subscriber::spawn());

    registry
        .with(fmt_layer_stdout)
        .with(fmt_layer_logfile)
        .init();

    Ok(guard)
}
