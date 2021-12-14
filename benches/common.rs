#![allow(dead_code)]

use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::Subscriber;
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::{writer::MutexGuardWriter, MakeWriter};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber_serde::{
    time::SystemClock,
    writer::{FlushGuard, NonBlocking},
    SerdeFormat, SerdeLayer, WriteEvent,
};

pub struct InMemoryWriter {
    inner: Arc<Mutex<Vec<u8>>>,
}

pub struct InMemoryWriterFlushGuard {
    inner: Arc<Mutex<Vec<u8>>>,
    dest: PathBuf,
}

const MB: usize = 0xfffff;
const WRITE_BUF_SIZE: usize = 200 * MB;

impl InMemoryWriter {
    pub fn new(p: Option<impl AsRef<Path>>) -> (Self, Option<InMemoryWriterFlushGuard>) {
        let inner = Arc::new(Mutex::new(Vec::with_capacity(WRITE_BUF_SIZE)));
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

impl WriteEvent for InMemoryWriter {
    fn write(&self, fmt: impl SerdeFormat, record: impl Serialize) -> io::Result<()> {
        let buf = &mut *self.inner.lock().unwrap();
        fmt.serialize(buf, record)
    }
}

impl Drop for InMemoryWriterFlushGuard {
    fn drop(&mut self) {
        let buf = self.inner.lock().expect("poisoned");
        std::fs::write(&self.dest, buf.as_slice()).unwrap();
    }
}

pub fn setup_tsjson_nb() -> (impl Subscriber + Send + Sync + 'static, WorkerGuard) {
    let (writer, g) = NonBlockingBuilder::default()
        .lossy(false)
        .finish(Vec::<u8>::with_capacity(WRITE_BUF_SIZE));

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

pub fn setup_jsonfull_nb() -> (impl Subscriber + Send + Sync + 'static, FlushGuard) {
    let (writer, g) = NonBlocking::new().finish(Vec::<u8>::with_capacity(WRITE_BUF_SIZE));

    let s = tracing_subscriber::registry().with(
        SerdeLayer::new()
            .with_writer(writer)
            .with_clock(SystemClock::default())
            .with_source_location(false)
            .with_span_events(FmtSpan::FULL)
            .finish(),
    );
    (s, g)
}

pub fn setup_tsjson(
    filepath: Option<impl AsRef<Path>>,
) -> (
    impl Subscriber + Send + Sync + 'static,
    Option<InMemoryWriterFlushGuard>,
) {
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

pub fn setup_jsonfull(
    filepath: Option<impl AsRef<Path>>,
) -> (
    impl Subscriber + Send + Sync + 'static,
    Option<InMemoryWriterFlushGuard>,
) {
    let (writer, g) = InMemoryWriter::new(filepath);

    let s = tracing_subscriber::registry().with(
        SerdeLayer::new()
            .with_writer(writer)
            .with_clock(SystemClock::default())
            .with_source_location(false)
            .with_span_events(FmtSpan::FULL)
            .finish(),
    );
    (s, g)
}

pub mod workloads {
    use tracing::*;

    pub fn simple(iters: usize) {
        for _ in 0..iters {
            let _outer = warn_span!("outer", x = 6).entered();
            for i in 0..10 {
                let _a = error_span!("a", i, p = "egg").entered();
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

    pub fn deeply_nested((depth, iters): (usize, usize)) {
        let mut spans = Vec::with_capacity(depth);

        for k in 0..depth {
            let s = warn_span!("egg", d = k, hello = "world").entered();
            spans.push(s);
        }
        for _ in 0..iters {
            error!(whatever = "shall", we = "do", x = 23, "oh no");
        }
        for s in spans.drain(..).rev() {
            drop(s);
        }
    }
}
