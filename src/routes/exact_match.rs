use axum::{Json, http::StatusCode, extract::State};
//use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};

use crate::{io::EngineInputData, AppState};



#[derive(Debug, Deserialize)]
pub struct ExactMatchRequest {
    strings: Vec<String>,
}

// the output response
#[derive(Serialize)]
pub struct ExactMatchResponse {
    matches: Vec<Option<EngineInputData>>
}

pub async fn exact_match(
    State(appstate): State<AppState>, 
    Json(payload): Json<ExactMatchRequest>,
    )
-> (StatusCode, Json<ExactMatchResponse>) {

    let input_vec = payload.strings;

    let mut result: Vec<Option<EngineInputData>> = Vec::new();
    for s in input_vec {
        result.push(appstate.db_hashmap.get(&s).cloned());
    }

    return (StatusCode::OK, Json(ExactMatchResponse { matches: result }));
}
