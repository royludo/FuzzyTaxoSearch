use std::{fs::File, collections::{HashSet, HashMap}, io::BufReader, sync::{Arc, Mutex}, borrow::BorrowMut, cell::RefCell, rc::Rc};

use axum::{Router, routing::{get, post}, Json, http::StatusCode, extract::State};
use clap::Parser;
use nucleo::Nucleo;
use nucleo_matcher::Utf32String;
use serde::{Deserialize, Serialize};

mod engine;
mod io;


//use crate::engine::EngineWrapper;

use crate::engine::*;
use crate::io::EngineInputData;



#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[arg(short= 'i', long = "input", value_parser = valid_file)]
    json_input: String,
}

fn valid_file(s: &str) -> Result<String, String> {
    // simply check if string is an openable file
    match File::open(s) {
        Ok(_) => Ok(s.to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn parse_taxa(filename: String) -> Result<HashSet<String>, String> {
    
    let mut rdr = csv::Reader::from_reader(BufReader::new(File::open(filename).unwrap()));

    let mut species_names: Vec<String> = Vec::new();
    let mut raw_species_name_set: HashSet<String> = HashSet::new();
    let mut parsed_species_name_set: HashSet<String> = HashSet::new();
    let mut name_map: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (i, record_result) in rdr.records().enumerate() {
        // The iterator yields Result<StringRecord, Error>, so we check the
        // error here.
        let record = match record_result {
            Ok(r) => r,
            Err(e) => return Err(format!("{}\nWhile parsing taxa file at line {}",e.to_string(), i)),
        };
        //println!("{:?}", record);

        let species_name = record[2].to_owned();

        species_names.push(species_name.clone());
        raw_species_name_set.insert(species_name.clone());
        let name_parts = species_name.split_whitespace().collect::<Vec<&str>>();
        let parsed_species_name = format!("{} {}", name_parts[0], name_parts[1]);
        parsed_species_name_set.insert(parsed_species_name.clone());

        // TODO bad change to get option
        if name_map.contains_key(&name_parts[0].to_owned().to_lowercase()) {
            let v = name_map.get_mut(&name_parts[0].to_owned().to_lowercase()).unwrap();
            v.push((parsed_species_name.clone(), record[0].to_owned()));
        }
        else {
            name_map.insert(name_parts[0].to_owned().to_lowercase(), vec![(parsed_species_name.clone(), record[0].to_owned())]);
        }


        if name_map.contains_key(&name_parts[1].to_owned().to_lowercase()) {
            let v = name_map.get_mut(&name_parts[1].to_owned().to_lowercase()).unwrap();
            v.push((parsed_species_name.clone(), record[0].to_owned()));
        }
        else {
            name_map.insert(name_parts[1].to_owned().to_lowercase(), vec![(parsed_species_name.clone(), record[0].to_owned())]);
        }

        /*println!("name map: {:?}", name_map);
        break;*/

        //name_map.insert(name_parts[0].to_owned(), (parsed_species_name.clone().to_lowercase(), record[0].to_owned()));
        //name_map.insert(name_parts[1].to_owned(), (parsed_species_name.to_lowercase(), record[0].to_owned()));



        /*if i > 3 {
            break;
        }*/

        //i += 1;
    }

    return Ok(parsed_species_name_set);
}

// the input request
#[derive(Debug, Deserialize)]
struct FuzzyMatchRequest {
    string: String,
}

// the output response
#[derive(Serialize)]
struct FuzzyMatchResponse {
    matches: Vec<EngineInputData>
}

#[derive(Clone)]
struct AppState {
    engine: Arc<Mutex<Nucleo<String>>>
}

async fn fuzzy(State(engine_pool): State<EnginePool>, Json(payload): Json<FuzzyMatchRequest>) -> (StatusCode, Json<FuzzyMatchResponse>) {
    //println!("{:?}", payload);
    let input = payload.string;
    println!("Received input: {:?}", input);

    if input.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(FuzzyMatchResponse{ matches: vec![] }));
    }

    println!("-- fuzzy request handler      {:?}", engine_pool.status());

    let result = engine_pool.get().await.unwrap().fuzzy_match(input);
    /*for r in result {
        println!("{:?} {:?}", r.data, r.matcher_columns);
    }*/

    //self.prev_search_str = input.to_owned();
    //return Ok(result);

    return (StatusCode::OK, Json(FuzzyMatchResponse{ matches: result }));
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    println!("Json file location: {:?}", args.json_input);

    let json_input = io::from_file(args.json_input);
    

    //let species_name_set = parse_taxa(args.taxa_file).unwrap();
    //println!("species count: {}", species_name_set.len());

    //let engine_wrapper = EngineWrapper::new(&species_name_set);
    /*let engine_pool = EnginePool::builder(PoolManager { input_data: result })
        .max_size(2)
        .build()
        .unwrap();*/
    let engine_pool = EnginePool::new(10);
    for _i in 0..2 {
        let _ = engine_pool.add(EngineWrapper::new(&json_input)).await;
    }
    println!("{:?}", engine_pool.status());

    
    

    //let appstate = AppState{ engine: Arc::new(Mutex::new(matcher)) };

    // build our application with a single route
    let app = Router::new()
        .route("/fuzzy", post(fuzzy)).with_state(engine_pool);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}