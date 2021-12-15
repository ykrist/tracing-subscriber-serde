mod common;
use common::*;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

macro_rules! benchmark {
  ($c:ident, $workload:path, $($input:expr),+) => {
    $(
      benchmark!(@IMPL $c, setup_serde_json_nb, "NB/SerdeJson", $workload, $input);
      benchmark!(@IMPL $c, setup_tsjson_nb, "NB/FmtLayer", $workload, $input);
    )*
  };

  (@IMPL $c:ident, $setup:path, $method:literal, $workload:path, $input:expr) => {
    let input = $input;
    let input_desc = format!("{:?}", &input);
    let (subscriber, _guard) = $setup();
    let benchmark_id = BenchmarkId::new(concat!($method, "/", stringify!($workload)), &input_desc);
    tracing::subscriber::with_default(subscriber, || {
      $c.bench_with_input(benchmark_id, &input, |b, &i| b.iter(||
        $workload(i)
      ));
    });

  };
}

fn comparison(c: &mut Criterion) {
    benchmark!(c, workloads::simple, 5, 10);
    benchmark!(c, workloads::deeply_nested, (15, 10));
    benchmark!(c, workloads::long_strings, 5, 10);
}

criterion_group!(benches, comparison);
criterion_main!(benches);
