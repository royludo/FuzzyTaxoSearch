use std::{fs::File, sync::{Arc, Mutex}};

use axum::{Router, routing::post};
//use axum_macros::debug_handler;
use clap::Parser;
use futures_delay_queue::DelayQueue;
use futures_intrusive::buffer::GrowingHeapBuf;
use time::Duration;
use tower_sessions::{MemoryStore, SessionManagerLayer, Expiry};
use uuid::Uuid;

mod engine;
mod io;
mod routes;


//use crate::engine::EngineWrapper;

use crate::engine::*;

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
        .route("/fuzzy", post(routes::fuzzy_autocomplete::fuzzy))
        .route("/exact_match", post(routes::exact_match::exact_match))
        .layer(session_layer)
        .with_state(appstate);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}