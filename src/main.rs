use std::{fs::File, sync::{Arc, Mutex}, str::FromStr};

use axum::{Router, routing::post, Json, http::StatusCode, extract::State};
//use axum_macros::debug_handler;
use clap::Parser;
use futures_delay_queue::DelayQueue;
use futures_intrusive::buffer::GrowingHeapBuf;
use serde::{Deserialize, Serialize};
use time::Duration;
use tower_sessions::{MemoryStore, SessionManagerLayer, Expiry, Session};
use uuid::Uuid;

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
    server_config: ServerConfig,

    autocomplete_engine_pool: EnginePool,
    autocomplete_used_engines: UsedEngineMap, // this thing could become the bottleneck
    autocomplete_delay_q: Arc<Mutex<DelayQueue<Uuid, GrowingHeapBuf<Uuid>>>>,

    gp_engine_pool: EnginePool,
    gp_used_engines: UsedEngineMap,
    gp_delay_q: Arc<Mutex<DelayQueue<Uuid, GrowingHeapBuf<Uuid>>>>,
}

#[derive(Debug, Clone)]
struct ServerConfig {
    // engine pool specificly for autocomplete
    autocomplete_pool_max_size: usize,
    autocomplete_pool_min_size: usize,
    session_expiry_delay: u64, // in seconds
    /*
        We absolutely want to avoid valid sessions trying to use expired engines that have been returned back to the pool.
        So the delay for returning en engine should be more than the expiry of a session.
        On the other hand it is ok if a usable engine sits unused because its session has expired.
        Total expiry of an engine will be session_expiry_delay + engine_returned_additional_delay.
     */
    engine_returned_additional_delay: u64, // in seconds

    // general purpose engine pool for other functions
    gp_pool_max_size: usize,
    gp_pool_min_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { 
            autocomplete_pool_max_size: 10,
            autocomplete_pool_min_size: 2,
            session_expiry_delay: 10,
            engine_returned_additional_delay: 2,
            gp_pool_max_size: 10,
            gp_pool_min_size: 2,
        }
    }
}

impl ServerConfig {
    fn get_engine_expiry(&self) -> u64 {
        return self.session_expiry_delay + self.engine_returned_additional_delay;
    }
}


//#[debug_handler]
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

    {   // the block is necessary because we aquire a lock that needs to go out of scope to be released
        println!("-- fuzzy request handler      EnginePool {:?} used_engines {:?}", appstate.autocomplete_engine_pool.status(), 
        appstate.autocomplete_used_engines.lock().await.len());
    }

    match session.id() {
        Some(sid) => {
            // follow up requests, session already created, need to reuse it
            println!("Follow up req, use sid {:?}", sid);

            let local_uuid_string: String = session.get(SESSION_ENGINE_KEY).await.unwrap().unwrap();
            // keep session alive by resetting expiry
            let local_uuid = Uuid::from_str(&local_uuid_string).unwrap();
            let mut used_engines = appstate.autocomplete_used_engines.lock().await;/* {
                Ok(used_engines_map) => used_engines_map,
                Err(e) => {
                    println!("Used engine lock error: {}", e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(FuzzyMatchResponse{ matches: vec![] }));
                } 
            };*/

            // we need to get ownership of delay_handle, hence the remove()
            let (mut session_engine, delay_handle) = used_engines.remove(&local_uuid).unwrap();
            let result = session_engine.fuzzy_match(input);

            //let stuff = delay_handle.borrow_mut();
            session.set_expiry(Some(Expiry::OnInactivity(Duration::seconds(appstate.server_config.session_expiry_delay as i64))));
            let new_handle = delay_handle.reset(
                std::time::Duration::from_secs(appstate.server_config.get_engine_expiry()))
                .await.unwrap();
            used_engines.insert(local_uuid, (session_engine, new_handle));



            return (StatusCode::OK, Json(FuzzyMatchResponse{ matches: result }));

        },
        None => {
            // first request, session not fully created yet
            println!("First req");
            let mut session_engine = appstate.autocomplete_engine_pool.remove().await.unwrap();/* {
                Ok(engine) => {
                    engine
                },
                Err(e) => {
                    // no more engine in pool, or other error
                    println!("EnginePool error: {}", e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(FuzzyMatchResponse{ matches: vec![] }));
                } 
            };*/

            // local id
            let uuid = Uuid::new_v4();

            // attribute this engine to the session
            
            /* {
                Ok(used_engines_map) => used_engines_map,
                Err(e) => {
                    println!("Used engine lock error: {}", e.to_string());
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(FuzzyMatchResponse{ matches: vec![] }));
                } 
            };*/

            
            //std::mem::drop(used_engines);
            

            let result = session_engine.fuzzy_match(input);

            session.insert(SESSION_ENGINE_KEY, uuid.to_string()).await.unwrap();
            let delay_handle = appstate.autocomplete_delay_q.lock().unwrap().insert(
                uuid, 
                std::time::Duration::from_secs(appstate.server_config.get_engine_expiry()));

            let mut used_engines = appstate.autocomplete_used_engines.lock().await;
            used_engines.insert(uuid, (session_engine, delay_handle));

            

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

async fn engine_cleanup_handler(
    rx: DelayQRx,
    arcmutex_used_engine: UsedEngineMap,
    engine_pool: EnginePool,
) {
    //let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        println!("awaiting timers");
        match rx.receive().await {
            Some(uuid_to_remove) => {
                println!("Putting back engine id {:?}", uuid_to_remove);
                let mut lock = arcmutex_used_engine.lock().await;
                let (engine, _) = lock.remove(&uuid_to_remove).unwrap();
                let _ = engine_pool.add(engine).await;
            },
            None => {
                // the channel was closed
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    println!("Json file location: {:?}", args.json_input);

    let json_input = io::from_file(args.json_input);
    
    let server_config = ServerConfig::default(); 

    // build autocomplete engine pool
    let (autocomplete_engine_pool, 
        arcmut_autocmplt_used_engine, 
        autocomplete_delay_queue, 
        autocomplete_rx) = build_pool_ecosystem(
        &json_input, 
        server_config.autocomplete_pool_max_size, 
        server_config.autocomplete_pool_min_size).await;
    
    // build general purpose engine pool
    let (gp_engine_pool, 
        arcmut_gp_used_engine, 
        gp_delay_queue, 
        gp_rx) = build_pool_ecosystem(
        &json_input, 
        server_config.gp_pool_max_size, 
        server_config.gp_pool_min_size).await;


    let appstate = AppState {
        server_config: server_config.clone(),
        autocomplete_engine_pool: autocomplete_engine_pool.clone(), 
        autocomplete_used_engines: arcmut_autocmplt_used_engine.clone(),
        autocomplete_delay_q: autocomplete_delay_queue,
        gp_engine_pool: gp_engine_pool.clone(),
        gp_used_engines: arcmut_gp_used_engine.clone(),
        gp_delay_q: gp_delay_queue,
    };

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // TODO why is session not working without this, and only when false ?
        .with_expiry(Expiry::OnInactivity(Duration::seconds(server_config.session_expiry_delay as i64)));

    let _ = tokio::spawn(engine_cleanup_handler(autocomplete_rx, arcmut_autocmplt_used_engine, autocomplete_engine_pool));
    let _ = tokio::spawn(engine_cleanup_handler(gp_rx, arcmut_gp_used_engine, gp_engine_pool));
    //let _ = forever.await;
    

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