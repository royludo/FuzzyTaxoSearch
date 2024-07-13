use clap::Parser;
use inquire::{Text, Autocomplete};
use reqwest::blocking::Client;
use serde::{Serialize, Deserialize};


#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[arg(long = "address")]
    address: String,
}


// the output response
#[derive(Deserialize)]
struct FuzzyMatchResponse {
    matches: Vec<String>
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
                let jsonres = response.json::<FuzzyMatchResponse>();
                return Ok(jsonres.unwrap().matches);
            },
            Err(e) => {
                println!("Request error, status: {:?} {:?}", e.status(), e);
                return Err(e.into());
            },
        }
        
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<inquire::autocompletion::Replacement, inquire::CustomUserError> {

        Ok(match highlighted_suggestion {
            Some(suggestion) => inquire::autocompletion::Replacement::Some(suggestion),
            None => None,
        })
    }
}


pub fn client_tui() {
    let args = Args::parse();
    println!("Host at: {:?}", args.address);
    let full_route = format!("http://{}/fuzzy", args.address);
   


    let client = reqwest::blocking::Client::new();
    let spesugg = SpeciesSuggesterRemote{ client, addr: full_route };


    let name = Text::new("Find species name: ").with_autocomplete(spesugg);



    match name.prompt() {
        Ok(name) => println!("Hello {}", name),
        Err(e) => {
            println!("An error happened when asking for your name, try again later.");
            println!("{}", e.to_string());
            },
    }
}
