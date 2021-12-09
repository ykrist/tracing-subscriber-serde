use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber_serde::{SerdeLayer, time::SystemClock, SpanEvents};

mod common;
use common::*;

fn main() -> anyhow::Result<()> {
  tracing_subscriber::registry()
      .with(SerdeLayer::new()
        .with_clock(SystemClock::default())
        .with_time_spans(true)
        .with_source_location(true)
        .with_span_events(SpanEvents::FULL)
        .with_thread_info(true, true)
        .with_writer(std::io::stdout())
        .finish())
      .with(EnvFilter::from_default_env())
      .init();

  creates_spans_and_events();
  Ok(())
}
