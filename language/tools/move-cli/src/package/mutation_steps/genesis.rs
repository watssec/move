use crate::package::mutation::*;
use crate::package::mutation_steps::utils::*;
use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use move_compiler::shared::Identifier;
use move_ir_types::location::*;
use move_package::{BuildConfig, ModelConfig};
use move_prover::cli::Options;
use pbr::ProgressBar;
use std::assert;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::path::Path;

pub fn run_mutation_genesis(
    path: &Path,
    mut config: BuildConfig,
    target_filter: &Option<String>,
    options: &Options,
) -> anyhow::Result<()> {
    let mut init_flag = false;

    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = write_create_if_not_exists(&evolution_status_file_path)?;


    let mut env_diags_map: BTreeMap<Loc, String> = BTreeMap::new();
    let mut diags_vec: Vec<String> =read_only_return_json_file(&env_diags_map_file_path)?;

    let mut mutate_loc_original: Vec<Loc> = read_only_return_json_file(&mutate_loc_original_file_path)?;

    let mut cnt = 0;
    for loc in mutate_loc_original {
        env_diags_map.insert(loc.clone(), diags_vec[cnt].clone());
        cnt = cnt + 1;
    }

    let evolution_status_keys: Vec<String> = evolution_status.clone().into_keys().collect();
    let mut bar_length = evolution_status_keys.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");
    for key in evolution_status_keys {
        println!("key{:?}",&key);
        let vec_vec_loc = evolution_status.get(&key).unwrap();

        for vec_loc in vec_vec_loc.clone() {
            let (mut env, targets) = prepare(
                config.clone(),
                path,
                target_filter,
                &options,
                &init_flag,
                vec_loc.clone(),
            )?;
            let error_vec = prove(&options, &env, &targets, genesis_round_counter)?;
            let current_function_name = env.current_function.unwrap().value().as_str().to_owned();
            let current_module_name = env.current_module.unwrap().value().as_str().to_owned();
            let current_appendix = env.appendix.clone();

            let time = Utc::now();
            let mut mutation_id = 0;
            let mut evolution_info = EvolutionInfo {
                function_id: current_function_name,
                module_id: current_module_name,
                evolution_round: 0,
                mutation_id,
                error: vec![],
                appendix: current_appendix,
                fin_sig: false,
                timestamp: time.to_rfc3339(),
            };
            println!("env{:?}",&env);
            mutation_id = mutation_id + 1;
            let genesis_round_counter: usize = 0;
            // read the serde vec and then rewrite it when it's not empty (this should be a vector)
            let mut original_evolution_info: Vec<EvolutionInfo> = write_create_if_not_exists(&evolution_info_file_path)?;


            if error_vec.is_empty() {
                evolution_info.fin_sig = true;
                original_evolution_info.push(evolution_info.clone());
                error_report_file_generation(&env, env_diags_map.clone(), vec_loc.clone());
            } else {
                evolution_info.error = error_vec.clone();
                original_evolution_info.push(evolution_info.clone());
            }

            // update the info into the evolution info file
            let serde_evolution_info = serde_json::to_value(original_evolution_info).unwrap();

            // delete the evolution info file first before renewing it

            fs::remove_file(&evolution_info_file_path)?;
            let mut evolution_info_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&evolution_info_file_path)
                .unwrap();
            serde_json::to_writer_pretty(&evolution_info_file, &serde_evolution_info)?;
        }
        pb.inc();
    }

    // if the mutated result passed the prover
    // 1) -> label it as passed
    // 2) -> record it into files when necessary

    pb.finish_print("Genesis iteration done");
    Ok(())
}
