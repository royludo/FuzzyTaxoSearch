use std::{fs::File, io::BufReader};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EngineInputData {
    pub string: String, // non-normalized, arbitrary length, utf8 string, can have whitespace
    pub data: serde_json::Value, // arbitrary data associated to it
}

pub fn from_file(filename: String) -> Vec<EngineInputData> {
    let bufread = BufReader::new(File::open(filename).unwrap());

    let input_data: Vec<EngineInputData> = serde_json::from_reader(bufread).unwrap();

    //println!("{:?}", input_data);

    return input_data;
}