use crate::package::mutation::*;
use crate::package::mutation_steps::utils::*;
use chrono::Utc;
use move_compiler::shared::Identifier;
use move_ir_types::location::*;
use move_package::BuildConfig;
use move_prover::cli::Options;
use pbr::ProgressBar;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::path::Path;

//TODO: make this part able to resume
pub fn run_evolution_testing(
    path: &Path,
    mut config: BuildConfig,
    target_filter: &Option<String>,
    sub_options: &Options,
) -> anyhow::Result<()> {
    for i in 0..evolution_round {
        let mut env_diags_map: BTreeMap<Loc, String> = BTreeMap::new();
        let mut diags_vec: Vec<String> = read_only_return_json_file(&env_diags_map_file_path)?;

        let mut mutate_loc_original: Vec<Loc> =
            read_only_return_json_file(&mutate_loc_original_file_path)?;

        let mut env_diags_map_keys: Vec<Loc> = read_only_return_json_file(&env_diags_map_keys_file_path)?;
        let mut cnt = 0;
        for loc in env_diags_map_keys {
            env_diags_map.insert(loc.clone(), diags_vec[cnt].clone());
            cnt = cnt + 1;
        }
        let mut mutation_id = 0;
        // Read in the evolution_status info
        let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> =
            read_only_return_json_file(&evolution_status_file_path)?;
        let mut evolution_bar_length = evolution_status.len();
        let mut pb = ProgressBar::new(evolution_bar_length.try_into().unwrap());
        pb.format("╢▌▌░╟");
        for (function, mutation_vec) in evolution_status.clone() {
            for vec in mutation_vec {
                if vec.len() != i + 2 {
                    continue;
                }

                mutation_id = mutation_id + 1;
                let mut init_flag = false;
                let (mut env, targets) = prepare(
                    config.clone(),
                    path,
                    target_filter,
                    &sub_options,
                    &init_flag,
                    vec.clone(),
                )?;

                env.current_vec = vec.clone();
                env.genesis_flag = false;

                let current_function_name =
                    env.current_function.unwrap().value().as_str().to_owned();
                let current_module_name = env.current_module.unwrap().value().as_str().to_owned();

                let current_appendix = env.appendix.clone();
                let time = Utc::now();

                let mut current_mutation_type_vec: Vec<String> = Vec::new();

                for single_vec in vec.clone() {
                    let current_mutation_type: String = env_diags_map
                        .get(&(single_vec.unwrap()))
                        .unwrap()
                        .to_owned()
                        .to_owned();
                    current_mutation_type_vec.push(current_mutation_type);
                }

                let mut evolution_info = EvolutionInfo {
                    module_id: current_module_name.clone(),
                    function_id: current_function_name.clone(),
                    evolution_round: i + 1,
                    mutation_id: mutation_id - 1,
                    error: vec![],
                    appendix: current_appendix,
                    fin_sig: false,
                    timestamp: time.to_rfc3339(),
                    mutation_type: current_mutation_type_vec,
                };

                let mut original_evolution_info: Vec<EvolutionInfo> =
                    read_only_return_json_file(&evolution_info_file_path)?;

                let error_vec = prove(&sub_options, &env, &targets, i + 1)?;
                if error_vec.is_empty() {
                    evolution_info.fin_sig = true;
                    error_report_file_generation(&env, env_diags_map.clone(), vec.clone());
                    original_evolution_info.push(evolution_info.clone());
                } else {
                    // returns true when new error is discovered
                    let result1 = reward_check_1(
                        &error_vec,
                        &original_evolution_info,
                        &current_function_name,
                        &current_module_name,
                    );

                    // returns true when old message is overwritten
                    let mut clone_vec = vec.clone();
                    let result2 = reward_check_2(
                        &mut clone_vec,
                        &error_vec,
                        &original_evolution_info,
                        &evolution_status,
                    );

                    // if
                    if result1 || result2 {
                        evolution_info.fin_sig = false;
                    } else {
                        evolution_info.fin_sig = true;
                    }

                    evolution_info.error = error_vec.clone();

                    original_evolution_info.push(evolution_info.clone());
                };

                fs::remove_file(&evolution_info_file_path);
                write_pretty_json(&evolution_info_file_path, original_evolution_info);

                pb.inc();
            }
        }
    }
    Ok(())
}
