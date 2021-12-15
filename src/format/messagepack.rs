use super::*;

#[derive(Clone, Copy, Debug)]
/// Serialize events as a stream of binary [MessagePack](https://msgpack.org/) objects.
/// Serialization speed is the same as [`Json`], but with smaller messages.  The trade-off is human-readability, but
/// if you are planning to post-process your logs programmatically anyway, this format would be suitable.
///
/// Requires the **`messagepack`** crate feature to be enabled.
pub struct MessagePack;

impl SerdeFormat for MessagePack {
    fn message_size_hint(&self) -> usize {
        512
    }

    fn serialize(&self, buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
        use rmp::encode::ValueWriteError;
        use rmp_serde::encode::Error;
        let mut s = rmp_serde::Serializer::new(buf).with_struct_map();
        match event.serialize(&mut s) {
            Err(Error::InvalidValueWrite(e)) => match e {
                ValueWriteError::InvalidDataWrite(e)
                | ValueWriteError::InvalidMarkerWrite(e) => Err(e),
            },
            Ok(()) => Ok(()),
            Err(_) => unreachable!(),
        }
    }
}


#[cfg(feature="consumer")]
pub use consumer::MessagePackStream;

#[cfg(feature="consumer")]
mod consumer {
    use serde::Deserialize;
    use rmp_serde::decode::{
        Deserializer,
        ReadReader,
        Error as RmpError,
    };
    use super::*;
    use crate::consumer::*;
    use crate::Event;
    use std::io::{Read, self};
    
    /// A stream of [`Event`s](crate::Event) serialized in MessagePack format.
    /// 
    /// See [`IterFile`](crate::consumer::IterFile) or [`StreamFormat`](crate::consumer::StreamFormat) on
    /// how to create one.
    pub struct MessagePackStream<R: Read> {
        deserializer: Deserializer<ReadReader<R>>,
    }
    
    
    impl<R: Read> Iterator for MessagePackStream<R> {
        type Item = io::Result<Event>;
    
        fn next(&mut self) -> Option<Self::Item> {
            match Event::deserialize(&mut self.deserializer) {
                Ok(e) => Some(Ok(e)),
                Err(RmpError::InvalidDataRead(io_err)) 
                | Err(RmpError::InvalidMarkerRead(io_err)) => {
                    if io::ErrorKind::UnexpectedEof == io_err.kind() {
                        None
                    } else {
                        Some(Err(io_err))
                    }
                },
                err => {
                    err.unwrap();
                    unreachable!()
                }
            }
        }
    }

    impl<R: Read> StreamFormat<R> for MessagePack {
        type Stream = MessagePackStream<R>;
    
        fn iter_reader(&self, reader: R) -> Self::Stream {
            MessagePackStream{ 
                deserializer: Deserializer::new(reader)
            }
        }
    }
}

#[cfg(feature="consumer")]
#[test]
fn messagepack() {    
    super::tests::test_format(MessagePack);
}
