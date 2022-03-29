use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber_serde::{
    consumer::*, format::Json, time::SystemClock, SerdeLayer, SpanEvents,
};

mod common;
use common::*;
use std::sync::{Arc, Mutex};

fn main() -> anyhow::Result<()> {
    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));

    tracing_subscriber::registry()
        .with(
            SerdeLayer::new()
                .with_clock(SystemClock::default())
                .with_time_spans(true)
                .with_source_location(true)
                .with_span_events(SpanEvents::FULL)
                .with_thread_info(true, true)
                .with_span_ids(true)
                .with_writer(Arc::clone(&buffer))
                .finish(),
        )
        .init();

    creates_spans_and_events();

    let buffer = buffer.lock().expect("Mutex poisoned");

    let printer = PrettyPrinter::default()
        .show_target(false)
        .show_span_ids(true)
        .limit_spans(10); // show at most N spans per event

    for event in Json.iter_reader(buffer.as_slice()) {
        let event = event?;
        printer.print(&event);
    }

    Ok(())
}
