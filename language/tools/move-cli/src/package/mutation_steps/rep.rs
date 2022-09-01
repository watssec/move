use crate::package::mutation::*;
use crate::package::mutation_steps::utils::*;
use move_ir_types::location::*;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::path::Path;

pub fn run_rep_generation() -> anyhow::Result<()> {
    for i in 0..evolution_round {
        rep_set_generation(i);
    }
    Ok(())
}

pub fn rep_set_generation(round_id: usize) -> anyhow::Result<()> {
    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&evolution_status_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .write(true)
            .append(false)
            .create(true)
            .open(&evolution_status_file_path)
            .unwrap()
    };
    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file) {
        Ok(content) => evolution_status = content,
        Err(e) => {}
    }

    let mut env_diags_map: BTreeMap<Loc, String> = BTreeMap::new();
    let evolution_status_keys: Vec<String> = evolution_status.clone().into_keys().collect();
    let mut env_diags_map_file = if Path::new(&env_diags_map_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&env_diags_map_file_path)
            .unwrap()
    } else {
        panic!();
    };
    match serde_json::from_reader(&env_diags_map_file) {
        Ok(content) => env_diags_map = content,
        Err(e) => {}
    }

    let mutated_loc_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&mutated_file_path)
        .unwrap();
    let mut mutated_vec: Vec<Loc> = Vec::new();
    match serde_json::from_reader(&mutated_loc_file) {
        Ok(content) => mutated_vec = content,
        // Error means this file has not been created yet, so we don't need to do anything
        // as the vec has already been initialized
        Err(e) => {}
    }

    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file) {
        Ok(content) => evolution_status = content,
        Err(e) => {}
    }

    let two_step_type = vec![
        "ArithmeticOperator".to_string(),
        "Constant".to_string(),
        "BitOperator".to_string(),
        "CompareOperator".to_string(),
    ];
    let function_keys: Vec<String> = evolution_status.clone().into_keys().collect();

    // iterate through the functions
    for key in function_keys {
        let mut new_vec: Vec<Vec<Option<Loc>>> = Vec::new();
        let str_key = key.as_str();
        for vec in evolution_status.get(str_key).unwrap().clone() {
            // check whether it is one of the types
            let to_rep_loc = vec.last();
            let current_mutation_type: String = env_diags_map
                .get(&((to_rep_loc.unwrap()).unwrap()))
                .unwrap()
                .to_owned()
                .to_owned();
            if two_step_type.contains(&current_mutation_type) {
                continue;
            }
            let mut fin_flag = check_fin(vec.to_owned());
            // push vec into new_vec
            new_vec.push(vec.to_owned().clone());
            if fin_flag {
                continue;
            }
            if vec.len() < round_id + 1 {
                continue;
            }
            let mut current_vec = vec.clone();
            current_vec.push(vec.last().unwrap().clone());
            new_vec.push(current_vec);
        }
        evolution_status.insert(key.to_string(), new_vec.clone());
    }

    fs::remove_file(&evolution_status_file_path);
    let mut evolution_status_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&evolution_status_file_path)
        .unwrap();
    let pretty_evolution_status = serde_json::to_value(&evolution_status).unwrap();
    serde_json::to_writer_pretty(&evolution_status_file, &pretty_evolution_status)?;
    Ok(())
}
