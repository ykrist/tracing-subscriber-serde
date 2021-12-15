use super::*;

#[derive(Copy, Clone, Debug)]
/// Serialize each event using a compact JSON format, separated by newlines.
pub struct Json;

impl SerdeFormat for Json {
    fn message_size_hint(&self) -> usize {
        512
    }

    fn serialize(&self, mut buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
        serde_json::to_writer(&mut buf, &event)?;
        buf.write("\n".as_bytes())?;
        Ok(())
    }
}


#[cfg(feature="consumer")]
pub use consumer::JsonStream;

#[cfg(feature="consumer")]
mod consumer {
    use super::*;
    use crate::consumer::*;
    use crate::Event;
    use std::io::{Read, self};
    
    /// A stream of [`Event`s](crate::Event) serialized in JSON format.
    /// 
    /// Created with `Json.iter_file("file.json")` (see [`IterFile`](crate::consumer::IterFile))  or `Json.iter_reader(reader)`
    /// (see [`StreamFormat`](crate::consumer::StreamFormat))
    pub struct JsonStream<R: Read> {
        stream: serde_json::StreamDeserializer<'static, serde_json::de::IoRead<R>, Event>
    }

    impl<R: Read> Iterator for JsonStream<R> {
        type Item = io::Result<Event>;
    
        fn next(&mut self) -> Option<Self::Item> {
            self.stream.next().map(|r| r.map_err(From::from))
        }
    }

    impl<R: Read> StreamFormat<R> for Json {
        type Stream = JsonStream<R>;
    
        fn iter_reader(self, reader: R) -> Self::Stream {
            JsonStream{ 
                stream: serde_json::Deserializer::from_reader(reader).into_iter::<Event>()
            }
        }
    }
}


#[cfg(feature="consumer")]
#[test]
fn json() {
    super::tests::test_format(Json);
}
