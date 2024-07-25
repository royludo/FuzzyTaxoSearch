use std::str::FromStr;

use axum::{Json, http::StatusCode, extract::State};
//use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use time::Duration;
use tower_sessions::{Expiry, Session};
use uuid::Uuid;

use crate::{io::EngineInputData, AppState};


// the input request
#[derive(Debug, Deserialize)]
pub struct FuzzyAutocompleteRequest {
    string: String,
}

// the output response
#[derive(Serialize)]
pub struct FuzzyAutocompleteResponse {
    matches: Vec<EngineInputData>
}

//#[debug_handler]
pub async fn fuzzy_autocomplete(
    session: Session,
    State(appstate): State<AppState>, 
    Json(payload): Json<FuzzyAutocompleteRequest>,
    )
-> (StatusCode, Json<FuzzyAutocompleteResponse>) {
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
        return (StatusCode::BAD_REQUEST, Json(FuzzyAutocompleteResponse{ matches: vec![] }));
    }

    {   // the block is necessary because we aquire a lock that needs to go out of scope to be released
        println!("-- fuzzy request handler      EnginePool {:?} used_engines {:?}", appstate.autocomplete_engine_pool.status(), 
        appstate.autocomplete_used_engines.lock().await.len());
    }

    match session.id() {
        Some(sid) => {
            // follow up requests, session already created, need to reuse it
            println!("Follow up req, use sid {:?}", sid);

            let local_uuid_string: String = session.get(crate::SESSION_ENGINE_KEY).await.unwrap().unwrap();
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



            return (StatusCode::OK, Json(FuzzyAutocompleteResponse{ matches: result }));

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

            session.insert(crate::SESSION_ENGINE_KEY, uuid.to_string()).await.unwrap();
            let delay_handle = appstate.autocomplete_delay_q.lock().unwrap().insert(
                uuid, 
                std::time::Duration::from_secs(appstate.server_config.get_engine_expiry()));

            let mut used_engines = appstate.autocomplete_used_engines.lock().await;
            used_engines.insert(uuid, (session_engine, delay_handle));

            

            return (StatusCode::OK, Json(FuzzyAutocompleteResponse{ matches: result }));

        },
    };

    

    // TODO better manage potential pool error
   
    /*for r in result {
        println!("{:?} {:?}", r.data, r.matcher_columns);
    }*/

    //self.prev_search_str = input.to_owned();
    //return Ok(result);

    
}
