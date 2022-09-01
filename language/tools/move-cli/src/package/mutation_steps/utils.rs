use serde::de::{self, Expected, Unexpected};
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::path::Path;
use std::collections::{BTreeMap, HashMap};
use anyhow::anyhow;

pub static evolution_status_file_path: &str = "evolution_status.json";
pub static evolution_info_file_path: &str = "evolution_info.json";
pub static mutated_file_path: &str = "mutated_loc.json";
pub static env_diags_map_file_path: &str = "env_diags_map.json";
pub static evolution_round: usize = 2;
pub static mutate_loc_original_file_path: &str = "mutate_loc_original.json";
pub static env_function_map_file_path: &str = "env_function_map.json";

// All of the file operation: given the input file address, ->
// return the file content

pub fn read_only_return_json_file<T: de::DeserializeOwned> (file_path: &str) -> anyhow::Result<T> {
    let mut file_content = if Path::new(&file_path).exists() {
        OpenOptions::new().read(true).open(&file_path).unwrap()
    } else {
        panic!();
    };
    let content = serde_json::from_reader(&file_content)?;
    Ok(content)
}

// write into the file if exists, create if no such file exist
pub fn write_create_if_not_exists<T: de::DeserializeOwned> (file_path: &str) -> anyhow::Result<T>{
    let mut file_content = if Path::new(&file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(&file_path)
            .unwrap()
    };
    let content = serde_json::from_reader(&file_content)?;
    Ok(content)
}