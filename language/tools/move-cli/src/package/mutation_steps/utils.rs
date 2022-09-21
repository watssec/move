use anyhow::anyhow;
use serde::de::{self, Expected, Unexpected};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::path::Path;
use serde::Serialize;

pub static evolution_status_file_path: &str = "evolution_status.json";
pub static evolution_info_file_path: &str = "evolution_info.json";
pub static mutated_file_path: &str = "mutated_loc.json";
pub static env_diags_map_file_path: &str = "env_diags_map.json";
pub static evolution_round: usize = 2;
pub static mutate_loc_original_file_path: &str = "mutate_loc_original.json";
pub static env_function_map_file_path: &str = "env_function_map.json";
pub static env_function_map_keys_file_path: &str = "env_function_map_keys.json";
pub static env_diags_map_keys_file_path: &str = "env_function_map_keys.json";
pub static rep_target_map_file_path: &str = "rep_target_map.json";
pub static rep_target_map_key_file_path: &str = "rep_target_map_key.json";
// All of the file operation: given the input file address, ->
// return the file content

pub fn read_only_return_json_file<T: de::DeserializeOwned>(file_path: &str) -> anyhow::Result<T> {
    let mut file_content = if Path::new(&file_path).exists() {
        OpenOptions::new().read(true).open(&file_path).unwrap()
    } else {
        panic!();
    };
    let content = serde_json::from_reader(&file_content)?;
    Ok(content)
}


// Open the file and read it, if no such file exists, create a new one and return a initialized datatype



// write into the file if exists, create if no such file exist, and
pub fn write_pretty_json<T: Serialize>(file_path: &str, content: T) -> anyhow::Result<()> {
    let mut file_writer = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&file_path)
        .unwrap();
    let pretty_content = serde_json::to_value(&content).unwrap();
    serde_json::to_writer_pretty(&file_writer, &pretty_content)?;
    Ok(())
}

pub fn write_create_if_not_exists<T: de::DeserializeOwned>(file_path: &str) -> anyhow::Result<T> {
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

pub fn open_and_read_create_if_not_exists <T: for<'de> serde::Deserialize<'de>>(file_path: &str) -> Result<T, serde_json::Error> {
    let mut file_content = OpenOptions::new().read(true).write(true).append(false).create(true).open(&file_path).unwrap();
    let content = serde_json::from_reader(&file_content);
    content
}

