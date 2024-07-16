use std::{fs::File, collections::{HashSet, HashMap}, sync::{Arc, Mutex}, str::FromStr};

use axum::{Router, routing::{get, post}, Json, http::StatusCode, extract::State, response::IntoResponse};
use clap::Parser;
use nucleo::Nucleo;
use serde::{Deserialize, Serialize};
use time::Duration;
use tokio::sync::RwLock;
use tower_sessions::{MemoryStore, SessionManagerLayer, Expiry, Session, session::Id};
use uuid::Uuid;
use axum_macros::debug_handler;

mod engine;
mod io;


//use crate::engine::EngineWrapper;

use crate::engine::*;
use crate::io::EngineInputData;

const SESSION_ENGINE_KEY: &str = "engine";


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
    engine_pool: EnginePool,
    used_engines: Arc<RwLock<HashMap<Uuid, EngineWrapper>>> // this thing could become the bottleneck
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct SessionStuff(String);

#[debug_handler]
async fn fuzzy(
    session: Session,
    State(appstate): State<AppState>, 
    Json(payload): Json<FuzzyMatchRequest>,
    )
-> (StatusCode, Json<FuzzyMatchResponse>) {
    //println!("{:?}", payload);
    let input = payload.string;
    /*println!("Received input: {:?}", input);
    println!("Session: {:?}", session);
    println!("session content {:?}", session.get::<SessionStuff>("key").await);
    println!("session id {:?}", session.id());
    
    println!("Session after: {:?}", session);
    println!("session content {:?}", session.get::<SessionStuff>("key").await);
    println!("session id {:?}", session.id());*/
    //session.insert("key", SessionStuff("some stuff".to_owned())).await.unwrap();

    if input.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(FuzzyMatchResponse{ matches: vec![] }));
    }

    println!("-- fuzzy request handler      EnginePool {:?} used_engines {:?}", appstate.engine_pool.status(), 
        appstate.used_engines.read().await.len());

    match session.id() {
        Some(sid) => {
            // follow up requests, session already created, need to reuse it
            println!("Follow up req, use sid {:?}", sid);

            let local_uuid: String = session.get(SESSION_ENGINE_KEY).await.unwrap().unwrap();
            let mut used_engines = appstate.used_engines.write().await;/* {
                Ok(used_engines_map) => used_engines_map,
                Err(e) => {
                    println!("Used engine lock error: {}", e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(FuzzyMatchResponse{ matches: vec![] }));
                } 
            };*/

            let session_engine = used_engines.get_mut(&Uuid::from_str(&local_uuid).unwrap()).unwrap();
            let result = session_engine.fuzzy_match(input);
            return (StatusCode::OK, Json(FuzzyMatchResponse{ matches: result }));

        },
        None => {
            // first request, session not fully created yet
            println!("First req");
            let mut session_engine = match appstate.engine_pool.get().await {
                Ok(engine) => {
                    engine
                },
                Err(e) => {
                    // no more engine in pool, or other error
                    println!("EnginePool error: {}", e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(FuzzyMatchResponse{ matches: vec![] }));
                } 
            };

            // local id
            let uuid = Uuid::new_v4();

            // attribute this engine to the session
            
            let mut used_engines = appstate.used_engines.write().await;/* {
                Ok(used_engines_map) => used_engines_map,
                Err(e) => {
                    println!("Used engine lock error: {}", e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(FuzzyMatchResponse{ matches: vec![] }));
                } 
            };*/

            used_engines.insert(uuid, session_engine.clone());
            //std::mem::drop(used_engines);
            

            session.insert(SESSION_ENGINE_KEY, uuid.to_string()).await.unwrap();
            let result = session_engine.fuzzy_match(input);
            return (StatusCode::OK, Json(FuzzyMatchResponse{ matches: result }));

        },
    };

    

    // TODO better manage potential pool error
   
    /*for r in result {
        println!("{:?} {:?}", r.data, r.matcher_columns);
    }*/

    //self.prev_search_str = input.to_owned();
    //return Ok(result);

    
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
    let appstate = AppState{ engine_pool, used_engines: Arc::new(RwLock::new(HashMap::new())) };

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        //.with_secure(false);
        .with_expiry(Expiry::OnInactivity(Duration::seconds(10)));
    

    //let appstate = AppState{ engine: Arc::new(Mutex::new(matcher)) };

    // build our application with a single route
    let app = Router::new()
        .route("/fuzzy", post(fuzzy))
        .layer(session_layer)
        .with_state(appstate);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}