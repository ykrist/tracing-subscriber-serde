#![allow(dead_code)]

use std::sync::{Mutex, Arc};
use std::path::{PathBuf, Path};
use tracing_subscriber::fmt::{MakeWriter, writer::MutexGuardWriter};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber_json_full::JsonLayer;
use tracing::Subscriber;

pub struct InMemoryWriter {
  inner: Arc<Mutex<Vec<u8>>>,
}

pub struct InMemoryWriterFlushGuard {
  inner: Arc<Mutex<Vec<u8>>>,
  dest: PathBuf,
}


const MB: usize = 0xfffff;

impl InMemoryWriter {
  pub fn new(p: Option<impl AsRef<Path>>) -> (Self, Option<InMemoryWriterFlushGuard>) {
    let inner = Arc::new(Mutex::new(Vec::with_capacity(200 * MB)));
    let g = p.map(|p| InMemoryWriterFlushGuard {
      dest: p.as_ref().to_path_buf(),
      inner: Arc::clone(&inner),
    });
    let w = InMemoryWriter { inner };
    (w, g)
  }
}

impl<'a> MakeWriter<'a> for InMemoryWriter {
  type Writer = MutexGuardWriter<'a, Vec<u8>>;

  fn make_writer(&'a self) -> Self::Writer {
    self.inner.make_writer()
  }
}


impl Drop for InMemoryWriterFlushGuard {
  fn drop(&mut self) {
    let buf = self.inner.lock().expect("poisoned");
    std::fs::write(&self.dest, buf.as_slice()).unwrap();
  }
}


pub fn setup_tsjson(filepath: Option<impl AsRef<Path>>) -> (impl Subscriber + Send + Sync + 'static, Option<InMemoryWriterFlushGuard>) {
  let (writer, g) = InMemoryWriter::new(filepath);

  let l = tracing_subscriber::fmt::Layer::new()
    .json()
    .with_target(true)
    .with_span_list(true)
    .with_current_span(false)
    .with_span_events(FmtSpan::FULL)
    .with_writer(writer);

  let s = tracing_subscriber::registry().with(l);
  (s, g)
}


pub fn setup_jsonfull(filepath: Option<impl AsRef<Path>>) -> (impl Subscriber + Send + Sync + 'static, Option<InMemoryWriterFlushGuard>) {
  let (writer, g) = InMemoryWriter::new(filepath);

  let s = tracing_subscriber::registry()
    .with(JsonLayer::new()
      .with_writer(writer)
      .with_clock(tracing_subscriber_json_full::time::SystemClock::default())
      .source_location(false)
      .span_exit(true)
      .span_create(true)
      .span_close(true)
      .span_enter(true)
      .finish()
    );
  (s, g)
}

pub mod workloads {
  use tracing::*;

  pub fn simple(iters: usize) {
    for _ in 0..iters {
      let _outer = warn_span!("outer", x=6).entered();
      for i in 0..10 {
        let _a = error_span!("a", i, p="egg").entered();
        error!(cat = true, bacon = 4, foo = "mao", "hello");
        let _b = debug_span!("check_for_egg", i).entered();
        if i % 2 == 0 {
          info!("egg");
        } else {
          trace!("no egg")
        }
      }
    }
  }

  pub fn long_strings(iters: usize) {
    let s1: String = std::iter::repeat('x').take(100).collect();
    let s2: String = std::iter::repeat('y').take(200).collect();
    let _outer = warn_span!("outer", x=%s1, y=?s2).entered();
    for _ in 0..iters {
      error!(whatever="shall", we="do", x=23, %s1, ?s2);
    }
  }

  pub fn deeply_nested((depth, iters) : (usize, usize)) {
    let mut spans = Vec::with_capacity(depth);

    for k in 0..depth {
      let s = warn_span!("egg", d=k, hello="world").entered();
      spans.push(s);
    }
    for _ in 0..iters {
      error!(whatever="shall", we="do", x=23, "oh no");
    }
    for s in spans.drain(..).rev() {
      drop(s);
    }
  }

}

// pub fn creates_spans_and_events(iters: usize) {
//   use tracing::*;
//
//   let _outer = warn_span!("outer", x=6).entered();
//   // let _a = error_span!("a", i, p="egg").entered();
//   trace!("no egg");
//   // drop(_a);
// }