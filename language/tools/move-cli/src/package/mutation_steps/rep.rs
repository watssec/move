use crate::package::mutation::*;
use crate::package::mutation_steps::utils::*;
use move_ir_types::location::*;
use pbr::ProgressBar;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::path::Path;

pub fn run_rep_generation() -> anyhow::Result<()> {

    for i in 0..evolution_round {
        // For each round, use this function to generate set
        let mut evolution_status = rep_set_generation(i);

        fs::remove_file(&evolution_status_file_path);
        let mut evolution_status_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&evolution_status_file_path)
            .unwrap();
        let pretty_evolution_status = serde_json::to_value(&evolution_status).unwrap();
        serde_json::to_writer_pretty(&evolution_status_file, &pretty_evolution_status)?;
    }
    Ok(())
}

pub fn rep_set_generation(round_id: usize) -> BTreeMap<String, Vec<Vec<Option<Loc>>>> {
    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> =
        read_only_return_json_file(&evolution_status_file_path).unwrap();
    let mut env_diags_map: BTreeMap<Loc, String> = BTreeMap::new();
    let mut diags_vec: Vec<String> = read_only_return_json_file(&env_diags_map_file_path).unwrap();
    let mut env_diags_keys: Vec<Loc> = read_only_return_json_file(&env_diags_map_keys_file_path).unwrap();
    let mut cnt = 0;
    for loc in env_diags_keys {
        env_diags_map.insert(loc.clone(), diags_vec[cnt].clone());
        cnt = cnt + 1;
    }

    let two_step_type = vec![
        "ArithmeticOperator".to_string(),
        "Constant".to_string(),
        "BitOperator".to_string(),
        "CompareOperator".to_string(),
    ];
    let function_keys: Vec<String> = evolution_status.clone().into_keys().collect();
    let mut bar_length = function_keys.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");
    // iterate through the functions
    for key in function_keys {
        let mut new_vec: Vec<Vec<Option<Loc>>> = evolution_status.get(&key).unwrap().clone();
        let str_key = key.as_str();
        for vec in evolution_status.get(str_key).unwrap().clone() {
            // check whether it is one of the types
            let to_rep_loc = vec.last();
            let current_mutation_type: String = env_diags_map
                .get(&((to_rep_loc.unwrap()).unwrap()))
                .unwrap()
                .to_owned()
                .to_owned();
            if !two_step_type.contains(&current_mutation_type) {
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
        pb.inc();
    }

    evolution_status
}
