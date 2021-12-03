use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber_json_full::{JsonLayer, time::SystemClock};

// fn type_name_of_val<T>(_: &T) {
//   println!("{}", std::any::type_name::<T>());
// }

fn creates_spans_and_events() {
  use tracing::*;

  let _outer = warn_span!("outer", x=6).entered();
  for i in 0..10 {
    let _a = error_span!("a", i, p="egg").entered();
    error!(cat=true, bacon=4, foo="mao", "hello");
    let _b = debug_span!("check_for_egg", i).entered();
    if i % 2 == 0{
      info!("egg");
    } else{
      trace!("no\negg")
    }
  }
}

//
// fn read_and_print_log() -> anyhow::Result<()> {
//   let file = std::fs::File::open(LOG_FILE)?;
//   let stream = EventStream::new().reader(file);
//
//   let p = PrettyPrinter::default();
//
//   for event in stream {
//     p.print(&event?);
//   }
//   Ok(())
// }

fn main() -> anyhow::Result<()> {

  tracing_subscriber::registry()
      .with(JsonLayer::new()
        .with_clock(SystemClock::default())
        .time_spans(true)
        .source_location(false)
        .finish())
      .init();

  creates_spans_and_events();



  Ok(())
}
