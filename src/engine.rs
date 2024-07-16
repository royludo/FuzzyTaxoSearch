use std::{sync::{Mutex, Arc}, collections::{HashMap, HashSet}};

use deadpool::{managed, unmanaged};
use nucleo::Nucleo;
use nucleo_matcher::Utf32String;

use crate::io::EngineInputData;

/* 
behaves like an Arc<Mutex> (doc says: This struct can be cloned and transferred 
across thread boundaries and uses reference counting for its internal state.)
*/
pub type EnginePool = unmanaged::Pool<EngineWrapper>;

#[derive(Clone)]
pub struct EngineWrapper {
    engine: Arc<Mutex<Nucleo<EngineInputData>>>, // is arc mutex really needed here ?
    prev_search_str: String,
}

impl EngineWrapper {
    fn init_engine() -> Nucleo<EngineInputData> {
        return Nucleo::new(
            nucleo::Config::DEFAULT,
            Arc::new(|| /*println!("notified")*/{}),
            None,
            1,
        );
    }

    pub fn new(db_string: &Vec<EngineInputData>) -> Self {
        println!("Create new engine");
        let mut engine = EngineWrapper::init_engine();
        
        // populate the search set
        let injector = engine.injector();
        //species_name_set.into_iter().for_each(|species_name| { inject.push(species_name, |_, _| {}); });
        for item in db_string.into_iter() {
            injector.push(item.clone(), |input_data, buffer| {
                buffer[0] = Utf32String::Ascii(input_data.string.clone().into());
            });
        }

        return EngineWrapper { engine: Arc::new(Mutex::new(engine)), prev_search_str: String::new() };
    }

    pub fn fuzzy_match(&mut self, input: String) -> Vec<EngineInputData> {
        let mut nucleo_matcher = self.engine.lock().unwrap();
    
        // test if current input is an extension of previous output
        let is_string_extension = self.prev_search_str.len() > 1 && input[0..input.len()-1] == self.prev_search_str;

        nucleo_matcher.pattern.reparse(
            0, 
            input.as_str(), 
            nucleo_matcher::pattern::CaseMatching::Ignore, 
            nucleo_matcher::pattern::Normalization::Smart, 
            is_string_extension);
        
        
        //println!("Tick {i}");
        // make matcher work, loop until it finishes, then retrieve snapshot of the result
        while nucleo_matcher.tick(10).running {}

        //println!("Nucleo status after tick {:?}", status);
        //println!("result count {:?}", self.nucleo_matcher.snapshot().matched_item_count());
        let max_display_result = std::cmp::min(10, nucleo_matcher.snapshot().matched_item_count());
        let result = nucleo_matcher.snapshot().matched_items(0..max_display_result)
            .into_iter()
            .map(|item| item.data.clone() )
            .collect::<Vec<EngineInputData>>();

        self.prev_search_str = input.to_owned();
        return result;
    }
}


