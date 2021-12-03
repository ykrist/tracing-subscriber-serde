use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber_json_full::{JsonLayer, time::SystemClock};
use std::io::Cursor;
use std::sync::Mutex;

fn creates_spans_and_events() {
  use tracing::*;

  let _outer = warn_span!("outer", x=6).entered();
  for i in 0..3 {
    let _a = error_span!("a", i, p="egg").entered();
    error!(cat=true, bacon=4, foo="mao", "hello");
    let _b = debug_span!("check_for_egg", i).entered();
    if i % 2 == 0{
      info!("egg");
      error!(eggy="no")
    } else{
      trace!(foo=42.0, "no\negg");
      debug!(a=4, b=1.4);
    }
  }
}



fn main() -> anyhow::Result<()> {

  tracing_subscriber::registry()
      .with(JsonLayer::new()
        .with_clock(SystemClock::default())
        .time_spans(true)
        .source_location(true)
        .time_spans(true)
        .span_enter(true)
        .span_exit(true)
        .span_close(true)
        .span_create(true)
        // .with_writer(Mutex::new(Cursor::new(Vec::<u8>::new())))
        .with_writer(std::io::stdout)
        .finish())
      .with(EnvFilter::from_default_env())
      .init();

  creates_spans_and_events();



  Ok(())
}
