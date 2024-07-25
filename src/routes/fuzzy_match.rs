use axum::{Json, http::StatusCode, extract::State};
//use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{io::EngineInputData, AppState};



#[derive(Debug, Deserialize)]
pub struct FuzzyMatchRequest {
    strings: Vec<String>,
    #[serde(default = "default_to_one")]
    n_first_results: u32,
}

fn default_to_one() -> u32 {
    return 1;
}

/**
 * For each string queried, returns between 0 and at most n results.
 * n comes from the n_first_results parameter passed in the request.
 */
#[derive(Serialize)]
pub struct FuzzyMatchResponse {
    matches: Vec<Vec<EngineInputData>>
}

pub async fn fuzzy_match(
    State(appstate): State<AppState>, 
    Json(payload): Json<FuzzyMatchRequest>,
    )
-> (StatusCode, Json<FuzzyMatchResponse>) {

    let input_vec = payload.strings;
    let limit = payload.n_first_results as usize;
    if input_vec.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(FuzzyMatchResponse{ matches: vec![] }));
    }

    println!("-- fuzzy request handler      EnginePool {:?}", appstate.gp_engine_pool.status());
    
    let mut engine = appstate.gp_engine_pool.get().await.unwrap();
    let mut result: Vec<Vec<EngineInputData>> = Vec::new();
    for s in input_vec {
        if s.is_empty() {
            result.push(vec![]);
            continue;
        }

        let string_res = engine.fuzzy_match(s); //[0..payload.n_first_results as usize].to_owned();
        let result_count_limit = std::cmp::min(string_res.len(), limit);
        result.push(string_res[0..result_count_limit].to_owned());
    }

    return (StatusCode::OK, Json(FuzzyMatchResponse { matches: result }));
}
