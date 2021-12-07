use tracing_subscriber::fmt::MakeWriter;
use std::io::Write;
use std::path::Path;

#[path="../benches/common.rs"]
mod bench_utils;



#[test]
fn inmemory_writes_to_file() {
  let (writer, g) = bench_utils::InMemoryWriter::new(Some("test.txt"));
  let mut w = writer.make_writer();
  writeln!(&mut w, "hello world").unwrap();
  drop(w);
  drop(writer);
  if Path::new("test.txt").exists() {
    std::fs::remove_file("test.txt").unwrap();
    panic!("bug");
  }
  drop(g);
  let s = std::fs::read_to_string("test.txt").unwrap();
  std::fs::remove_file("test.txt").unwrap();
  assert_eq!(s, "hello world\n");
}

#[test]
fn same_number_of_records() {
  let (s, g) = bench_utils::setup_tsjson(Some("ts.json"));
  tracing::subscriber::with_default(s, || bench_utils::workloads::simple(5));
  drop(g);

  let (s, g) = bench_utils::setup_jsonfull(Some("full.json"));
  tracing::subscriber::with_default(s, || bench_utils::workloads::simple(5));
  drop(g);


  let ts_json = std::fs::read_to_string("ts.json").unwrap();
  let full_json = std::fs::read_to_string("full.json").unwrap();


  assert_eq!(ts_json.lines().count(), full_json.lines().count());

  std::fs::remove_file("ts.json").ok();
  std::fs::remove_file("full.json").ok();
}