use serde_json;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Error;
use std::io::prelude::*;
use std::path;

/// A structure to hold environment settings. Backed by settings.json file in same directory
#[derive(Serialize, Deserialize, Debug)]
pub struct Configs {
    pub verbose_mode: bool,
    pub working_directory: String,
    pub compress_rasters: bool,
    pub max_procs: isize,
}

impl Configs {
    pub fn new() -> Configs {
        Configs{ 
            verbose_mode: true,
            working_directory: String::new(),
            compress_rasters: true,
            max_procs: -1
        }
    }
}

pub fn get_configs() -> std::result::Result<Configs, Error> {
    let mut exe_path = std::env::current_dir()?.to_str().unwrap_or("No exe path found.").to_string();
    let plugin_dir = path::MAIN_SEPARATOR.to_string() + "plugins";
    if exe_path.ends_with(&plugin_dir) {
        exe_path = exe_path.replace(&plugin_dir, "");
    }
    let config_file = exe_path + &path::MAIN_SEPARATOR.to_string() + "settings.json";
    // let contents = fs::read_to_string(config_file).expect("Failed to open config_file.json file.");
    // let configs: Configs = serde_json::from_str(&contents).expect("Failed to parse config_file.json file.");
    let configs: Configs = match fs::read_to_string(config_file) {
        Ok(contents) => {
            serde_json::from_str(&contents).expect("Failed to parse config_file.json file.")
        }, 
        Err(_) => {
            Configs::new()
        }
    };
    Ok(configs)
}

pub fn save_configs<'a>(configs: &Configs) -> std::result::Result<(), Error> {
    let configs_json = serde_json::to_string_pretty(&configs).expect("Error converting Configs object to JSON.");
    let mut exe_path = std::env::current_dir()?.to_str().unwrap_or("No exe path found.").to_string();
    let plugin_dir = path::MAIN_SEPARATOR.to_string() + "plugins";
    if exe_path.ends_with(&plugin_dir) {
        exe_path = exe_path.replace(&plugin_dir, "");
    }
    let config_file = exe_path + &path::MAIN_SEPARATOR.to_string() + "settings.json";
    match File::create(config_file) {
        Ok(mut file) => {
            match file.write_all(configs_json.as_bytes()) {
                Ok(()) => {}, // do nothing
                Err(_e) => {
                    eprintln!("Error writing to output settings.json file, likely do to a permissions problem. Settings will not be updated.");
                }
            };
        },
        Err(_e) => { 
            eprintln!("Could not create output settings.json file. WBT is likely installed somewhere without write permission.")
        }
    };
    
    Ok(())
}

