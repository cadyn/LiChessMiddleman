extern crate buffer

use buffer::Buffer;
use std::str;
use rocket::serde::{Deserialize, Serialize};
use rocket::serde::json::{Json, Value, json};

pub trait JsonObjectStreamer {
    fn json_objects(&mut self) -> JsonObjects<Self>;
}

impl<T: 'a> JsonObjectStreamer for T {
    fn json_objects(&mut self) -> JsonObjects<T> {
        JsonObjects { reader: self }
    }
}

pub struct JsonObjects<'a, B> where B: 'a {
    reader: &'a mut B
}

impl<'a, B, T> Iterator for JsonObjects<'a, B, T> where B: Buffer + 'a {
    type Item = Json<T>;

    fn next(&mut self) -> Option<Json<T>> {
        let mut line_bytes = match self.reader.read_until(b'\r') {
            Ok(bytes) => bytes,
            Err(_)    => return None,
        };

        if line_bytes.last() == Some(&b'\r') {
            // drop the \r
            line_bytes.pop();

            // skip the \n
            match self.reader.read_char() {
                Ok(_)  => (),
                Err(_) => return None,
            }
        }

        let line = match str::from_utf8(&line_bytes) {
            Ok(line) => line,
            Err(_)   => return None
        };

        Json::from_str(line).ok()
    }
}