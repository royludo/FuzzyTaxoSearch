use clap::Parser;
use inquire::{Text, Autocomplete, Select, validator::Validation};
use reqwest::blocking::Client;
use serde::{Serialize, Deserialize};


#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[arg(long = "address")]
    address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EngineInputData {
    pub string: String, // non-normalized, arbitrary length, utf8 string, can have whitespace
    pub data: serde_json::Value, // arbitrary data associated to it
}

// the output response
#[derive(Deserialize)]
struct FuzzyAutocompleteResponse {
    matches: Vec<EngineInputData>
}

#[derive(Debug, Clone)]
struct SpeciesSuggesterRemote {
    client: Client,
    addr: String,
}
impl SpeciesSuggesterRemote {
    fn new(addr: String, client: Client) -> Self {
        return SpeciesSuggesterRemote { client, addr };
    }
}

impl Autocomplete for SpeciesSuggesterRemote {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        if input.is_empty() {
            return Ok(vec![]);
        }

        let result = self.client.post(self.addr.as_str())
            .header("Content-Type", "application/json")
            .body(format!("{{\"string\":\"{}\"}}", input))
            .send();



        match result {
            Ok(response) => {
                //println!("status: {:?}", response.status());
                //println!("body: {:?}", response.text().unwrap());
                let jsonres = response.json::<FuzzyAutocompleteResponse>();
                match jsonres {
                    Ok(inner) => {
                        return Ok(inner.matches.into_iter().map(|data| format!("{}    ::::    {}", data.string, data.data.to_string())).collect())
                    },
                    Err(e) => return Err(e.into()),
                }
                
            },
            Err(e) => {
                println!("Request error, status: {:?} {:?}", e.status(), e);
                return Err(e.into());
            },
        }
        
    }

    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {

        Ok(match highlighted_suggestion {
            Some(suggestion) => inquire::autocompletion::Replacement::Some(suggestion),
            None => None,
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ExactMatchRequest {
    strings: Vec<String>,
}

#[derive(Deserialize)]
pub struct ExactMatchResponse {
    matches: Vec<Option<EngineInputData>>
}


#[derive(Debug, Serialize)]
pub struct FuzzyMatchRequest {
    strings: Vec<String>,
    n_first_results: u32,
}

/**
 * For each string queried, returns between 0 and at most n results.
 * n comes from the n_first_results parameter passed in the request.
 */
#[derive(Deserialize)]
pub struct FuzzyMatchResponse {
    matches: Vec<Vec<EngineInputData>>
}


pub fn client_tui() {
    let args = Args::parse();
    println!("Host at: {:?}", args.address);
    let fuzzy_autocomplete_route = format!("http://{}/fuzzy", args.address);
    let exact_match_route = format!("http://{}/exact_match", args.address);
    let fuzzy_match_route = format!("http://{}/fuzzy_match", args.address);
   
    let client = reqwest::blocking::ClientBuilder::new().cookie_store(true).build().unwrap();

    let main_screen_fuzzy_auto_str = "Fuzzy autocomplete";
    let main_screen_exact_match_str = "Exact match";
    let main_screen_fuzzy_match_str = "Fuzzy match";
    let main_screen_options = vec![main_screen_fuzzy_auto_str, main_screen_exact_match_str, main_screen_fuzzy_match_str];

    let main_choice = Select::new("Choose what to test", main_screen_options).prompt().unwrap();

    if main_choice == main_screen_fuzzy_auto_str {

        let spesugg = SpeciesSuggesterRemote::new(fuzzy_autocomplete_route, client);

        let name = Text::new("Find species name: ").with_autocomplete(spesugg).prompt().unwrap();

        println!("\tResult: {}", name);

    } else if main_choice == main_screen_exact_match_str  {
        
        let query_string = Text::new("Batch exact match: ")
            .with_help_message("Multiple query strings possible, separated with a comma ','")
            .prompt()
            .unwrap();
        let query_strings = query_string.split(",").map(|s| s.trim().to_owned()) .collect::<Vec<String>>();

        println!("\tSend request:");
        println!("\tPOST to addr {}", exact_match_route);
        let req_payload = ExactMatchRequest{ strings: query_strings };
        println!("\tpayload: {}", serde_json::to_string(&req_payload).unwrap());
        
        let result = client.post(exact_match_route)
            .json(&req_payload)
            .send().unwrap();
        
        let jsonres = result.json::<ExactMatchResponse>().unwrap();
        println!("\tResponse OK: {:?}", jsonres.matches);
        

    } else if main_choice == main_screen_fuzzy_match_str  {
        
        let query_string = Text::new("Batch fuzzy match: ")
            .with_help_message("Multiple query strings possible, separated with a comma ','")
            .prompt()
            .unwrap();
        let query_strings = query_string.split(",").map(|s| s.trim().to_owned()) .collect::<Vec<String>>();

        let limit = Text::new("Limit each string results: ")
            .with_help_message("A positive number")
            .with_default("1")
            .with_validator(|input: &str| {
                match u32::from_str_radix(input, 10) {
                    Ok(n) => if n > 0 {
                        Ok(Validation::Valid)
                    } else {
                        Ok(Validation::Invalid("Should be a positive number".into()))
                    },
                    Err(_) => Ok(Validation::Invalid("Should be a number".into())),
                }
            })
            .prompt()
            .unwrap();


        println!("\tSend request:");
        println!("\tPOST to addr {}", fuzzy_match_route);
        let req_payload = FuzzyMatchRequest{ 
            strings: query_strings, 
            n_first_results: u32::from_str_radix(limit.as_str(), 10).unwrap() 
        };
        println!("\tpayload: {}", serde_json::to_string(&req_payload).unwrap());
        
        let result = client.post(fuzzy_match_route)
            .json(&req_payload)
            .send().unwrap();
        
        let jsonres = result.json::<FuzzyMatchResponse>().unwrap();
        println!("\tResponse OK: {:?}", jsonres.matches);
        

    }


    
}
