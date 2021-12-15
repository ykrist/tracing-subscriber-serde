//! Utilities and traits for storing and producing span timings and event timestamps.
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Timing information about a span's lifetime.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct SpanTime {
    pub(crate) busy: u64,
    pub(crate) idle: u64,
}

impl SpanTime {
    /// The time this span spent busy
    pub fn busy(&self) -> Duration {
        Duration::from_nanos(self.busy)
    }

    /// The time this span spent idle
    pub fn idle(&self) -> Duration {
        Duration::from_nanos(self.idle)
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct SpanTimer {
    busy: u64,
    idle: u64,
    last_update: Instant,
}

impl SpanTimer {
    pub fn new() -> Self {
        SpanTimer {
            busy: 0,
            idle: 0,
            last_update: Instant::now(),
        }
    }
}

impl SpanTimer {
    pub fn start_busy(&mut self) {
        let now = Instant::now();
        self.idle += now.duration_since(self.last_update).as_nanos() as u64;
        self.last_update = now;
    }

    pub fn end_busy(&mut self) {
        let now = Instant::now();
        self.busy += now.duration_since(self.last_update).as_nanos() as u64;
        self.last_update = now;
    }

    pub fn finish(&self) -> SpanTime {
        SpanTime {
            busy: self.busy,
            idle: self.idle,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// The UNIX epoch time, i.e the time since 00:00 1 Jan, 1970 (UTC).
///
/// This type almost identical to [`Duration`], but uses shorter field names for serialisation
/// to self-describing formats such as JSON.  It can be converted to and from [`Duration`]
/// and converted to [`SystemTime`].
pub struct UnixTime {
    // Number of seconds since 00:00 1 Jan, 1970 (UTC)
    #[serde(rename = "s")]
    seconds: u64,
    // Number of nanoseconds (seconds + nanoseconds = epoch time)
    #[serde(rename = "n")]
    nanos: u32,
}

impl From<Duration> for UnixTime {
    fn from(d: Duration) -> Self {
        UnixTime {
            seconds: d.as_secs(),
            nanos: d.subsec_nanos(),
        }
    }
}

impl From<UnixTime> for Duration {
    fn from(t: UnixTime) -> Self {
        Duration::new(t.seconds, t.nanos)
    }
}

impl From<UnixTime> for SystemTime {
    fn from(t: UnixTime) -> Self {
        let mut s = SystemTime::UNIX_EPOCH;
        s += Duration::from(t);
        s
    }
}

/// Tells the time in the only time worth telling: [`UnixTime`].
pub trait Clock {
    /// Get the current time for timestamping purposes.
    ///
    /// Returning `None` indicates no timestamp should be recorded.
    fn time(&self) -> Option<UnixTime>;
}

#[derive(Copy, Clone, Default)]
/// A [`Clock`] which uses [`SystemTime::now()`] to tell the time.
pub struct SystemClock {
    _private: (),
}

impl Clock for SystemClock {
    fn time(&self) -> Option<UnixTime> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(UnixTime::from)
    }
}

impl Clock for () {
    fn time(&self) -> Option<UnixTime> {
        None
    }
}
