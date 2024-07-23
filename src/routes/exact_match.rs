use axum::{Json, http::StatusCode, extract::State};
//use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::{io::EngineInputData, AppState};



#[derive(Debug, Deserialize)]
pub struct FuzzyMatchRequest {
    string: String,
}

// the output response
#[derive(Serialize)]
pub struct FuzzyMatchResponse {
    matches: Vec<EngineInputData>
}

pub async fn exact_match(
    session: Session,
    State(appstate): State<AppState>, 
    Json(payload): Json<FuzzyMatchRequest>,
    )
-> (StatusCode, Json<FuzzyMatchResponse>) {

    return (StatusCode::NOT_FOUND, Json(FuzzyMatchResponse { matches: vec![] }));
}
