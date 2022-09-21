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
    // The genesis status has been input into evolution_status from the init step

    /*
    initialize variables:
      - writable: evolution_status,
      - read-only: diags_vec, env_diags_keys
    */

    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> =
        write_create_if_not_exists(&evolution_status_file_path)?;
    let mut env_diags_map: BTreeMap<Loc, String> = BTreeMap::new();
    let mut diags_vec: Vec<String> = read_only_return_json_file(&env_diags_map_file_path)?;
    let mut env_diags_keys: Vec<Loc> = read_only_return_json_file(&env_diags_map_keys_file_path)?;

    let mut cnt = 0;
    for loc in env_diags_keys {
        env_diags_map.insert(loc.clone(), diags_vec[cnt].clone());
        cnt = cnt + 1;
    }
    let mut mutation_id = 0;

    let evolution_status_keys: Vec<String> = evolution_status.clone().into_keys().collect();
    let mut bar_length = evolution_status_keys.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");
    for key in evolution_status_keys {
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
            let current_function_name = env.current_function.unwrap().value().as_str().to_owned();
            let current_module_name = env.current_module.unwrap().value().as_str().to_owned();
            let current_appendix = env.appendix.clone();
            let genesis_round_counter: usize = 0;

            let error_vec = prove(&options, &env, &targets, genesis_round_counter)?;
            let time = Utc::now();
            let current_mutation_type: String = env_diags_map
                .get(&(vec_loc[0].unwrap()))
                .unwrap()
                .to_owned()
                .to_owned();

            let mut evolution_info = EvolutionInfo {
                function_id: current_function_name,
                module_id: current_module_name,
                evolution_round: 0,
                mutation_id,
                error: vec![],
                appendix: current_appendix,
                fin_sig: false,
                timestamp: time.to_rfc3339(),
                mutation_type: vec![current_mutation_type],
            };

            mutation_id = mutation_id + 1;

            // read the serde vec and then rewrite it when it's not empty (this should be a vector)
            let mut original_evolution_info: Vec<EvolutionInfo> =
                if Path::new(&evolution_info_file_path).exists() {
                    read_only_return_json_file(&evolution_info_file_path).unwrap()
                } else {
                    Vec::new()
                };

            if error_vec.is_empty() {
                evolution_info.fin_sig = true;
                original_evolution_info.push(evolution_info.clone());
                error_report_file_generation(&env, env_diags_map.clone(), vec_loc.clone());
            } else {
                evolution_info.error = error_vec.clone();
                original_evolution_info.push(evolution_info.clone());
            }
            fs::remove_file(&evolution_info_file_path);
            write_pretty_json(&evolution_info_file_path, original_evolution_info);

        }

        pb.inc();
    }

    // if the mutated result passed the prover
    // 1) -> label it as passed
    // 2) -> record it into files when necessary

    pb.finish_print("Genesis iteration done");
    Ok(())
}
