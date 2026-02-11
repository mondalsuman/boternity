//! Tracing subscriber initialization with structured logging and optional
//! OpenTelemetry trace export.
//!
//! # Usage
//!
//! ```no_run
//! // Basic structured logging only
//! boternity_observe::tracing_setup::init_tracing(false).unwrap();
//!
//! // With OpenTelemetry export to stdout (for local development)
//! boternity_observe::tracing_setup::init_tracing(true).unwrap();
//! ```

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use std::sync::OnceLock;

/// Stores the OTel tracer provider so it can be shut down cleanly on exit.
static TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

/// Initialize the global tracing subscriber.
///
/// - Always installs a structured `fmt` layer with target visibility and span
///   close timing.
/// - When `enable_otel` is true, additionally bridges tracing spans to
///   OpenTelemetry using a stdout exporter (suitable for local development;
///   swap the exporter for OTLP in production).
/// - Respects `RUST_LOG` via `EnvFilter::from_default_env()`.
///
/// # Errors
///
/// Returns an error if the global subscriber has already been set or if
/// the OTel pipeline fails to initialize.
pub fn init_tracing(enable_otel: bool) -> Result<(), Box<dyn std::error::Error>> {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_span_events(FmtSpan::CLOSE);

    let env_filter = EnvFilter::from_default_env();

    if enable_otel {
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
            .build();
        let tracer = provider.tracer("boternity");
        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

        // Store the provider for shutdown and register it globally.
        let _ = TRACER_PROVIDER.set(provider.clone());
        opentelemetry::global::set_tracer_provider(provider);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }

    Ok(())
}

/// Flush pending traces and shut down the OpenTelemetry tracer provider.
///
/// Call this before process exit to ensure all buffered spans are exported.
/// Safe to call even when OTel was not enabled (no-op in that case).
pub fn shutdown_tracing() {
    if let Some(provider) = TRACER_PROVIDER.get() {
        if let Err(e) = provider.shutdown() {
            eprintln!("Warning: OTel tracer provider shutdown error: {e}");
        }
    }
}
