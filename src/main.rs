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