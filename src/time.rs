use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use serde::{Deserialize, Serialize};

/// time * TIME_SCALE = number of seconds
pub const TIME_SCALE : u64 = 1000_000_000;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct SpanTime {
  busy: u64,
  idle: u64
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct SpanTimer {
  busy: u64,
  idle: u64,
  last_update: Instant,
}

impl SpanTimer {
  pub fn new() -> Self {
    SpanTimer{ busy: 0, idle: 0, last_update: Instant::now() }
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
    SpanTime { busy: self.busy, idle: self.idle }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnixTime {
  #[serde(rename="s")]
  pub seconds: u64,
  #[serde(rename="n")]
  pub nanos: u32,
}

impl UnixTime {
  pub fn from_duration(d: Duration) -> Self {
    UnixTime {
      seconds: d.as_secs(),
      nanos: d.subsec_nanos(),
    }
  }
}

pub trait Clock {
  fn get_time(&self) -> Option<UnixTime>;
}

#[derive(Copy, Clone, Default)]
pub struct SystemTimer {
  _private: ()
}


impl Clock for SystemTimer {
  fn get_time(&self) -> Option<UnixTime> {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .ok()
      .map(UnixTime::from_duration)
  }
}

impl Clock for () {
  fn get_time(&self) -> Option<UnixTime> {
    None
  }
}