// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Support for the mutation in the package system.
use crate::package::cli::MutationOptions;
use anyhow::{bail, Result};
use move_compiler::diagnostics::{self, codes, Diagnostics};
use move_package::{BuildConfig, ModelConfig};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::{io::Write, path::Path, time::Instant};
extern crate pbr;
use anyhow::anyhow;
use chrono::Utc;
use move_compiler::Flags;
use move_ir_types::location::*;
use move_model::{model::GlobalEnv, symbol::Symbol};
use move_prover::cli::Options;
use move_prover::{generate_boogie, verify_boogie};
use move_stackless_bytecode::{
    function_target_pipeline::FunctionTargetsHolder,
    pipeline_factory::default_pipeline_with_options,
};
use pbr::ProgressBar;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::copy;
use std::io::stdout;
use std::time::{Duration, SystemTime};
extern crate rustc_serialize;
use crate::package::mutation_steps::{
    combination::*, evolution::*, genesis::*, init::*, rep::*, mix::*, utils::*,
};

use move_compiler::parser::ast::FunctionName;
use move_compiler::shared::Identifier;
use rustc_serialize::json::Json;
use std::collections::HashSet;
use std::iter::FromIterator;
//use f::Result;
// =================================================================================================
// Running the mutation as a package command

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvolutionInfo {
    pub(crate) function_id: String,
    pub(crate) module_id: String,
    pub(crate) evolution_round: usize,
    pub(crate) mutation_id: usize,
    pub(crate) error: Vec<String>,
    pub(crate) appendix: Vec<String>,
    pub(crate) fin_sig: bool,
    pub(crate) timestamp: String,
    pub(crate) mutation_type: Vec<String>,
}

pub fn run_move_mutation(
    mut config: BuildConfig,
    path: &Path,
    target_filter: &Option<String>,
    for_test: bool,
    mut options: &Option<MutationOptions>,
) -> anyhow::Result<()> {
    //run the prover in dev mode so that addresses get default assignments
    config.dev_mode = true;

    let mut args = vec!["package".to_string()];
    let mutation_toml = Path::new(&path).join("Mutation.toml");
    if mutation_toml.exists() {
        args.push(format!("--config={}", mutation_toml.to_string_lossy()));
    }
    let mut vec_options: &[String] = &[];
    args.extend(vec_options.iter().cloned());

    let mut sub_options = move_prover::cli::Options::create_from_args(&args)?;
    match options {
        Some(MutationOptions::Init) => {
            run_mutation_init(path, config.clone(), target_filter, &sub_options)
        }
        Some(MutationOptions::Genesis) => {
            run_mutation_genesis(path, config.clone(), target_filter, &sub_options)
        }
        Some(MutationOptions::Rep) => run_rep_generation(),
        Some(MutationOptions::Combination) => run_normal_set_generation(),
        Some(MutationOptions::Evolution) => {
            run_evolution_testing(path, config.clone(), target_filter, &sub_options)
        },
        Some(MutationOptions::Mix) => {
            run_mix_generation()
        },
        _ => panic!(),
    };

    // import evolution_status
    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();
    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&evolution_status_file_path)
            .unwrap()
    } else {
        panic!();
    };
    match serde_json::from_reader(&evolution_status_file) {
        Ok(content) => evolution_status = content,
        Err(e) => {}
    }

    Ok(())
}

pub(crate) fn prepare(
    config: BuildConfig,
    path: &Path,
    target_filter: &Option<String>,
    options: &Options,
    init_flag: &bool,
    loc_vec: Vec<Option<Loc>>,
) -> Result<(GlobalEnv, FunctionTargetsHolder)> {
    let mut flags = Flags::empty();
    // if this is the init process
    if *init_flag {
        flags.mutation = false;
    } else {
        flags.mutation = true;

        for loc in loc_vec {
            let current_loc = loc.unwrap();
            flags.current_file_hash.push(current_loc.file_hash);
            flags.current_start.push(current_loc.start);
            flags.current_end.push(current_loc.end);
        }
    }

    let mut env = config.clone().move_model_for_package(
        flags,
        path,
        ModelConfig {
            all_files_as_targets: false,
            target_filter: target_filter.clone(),
        },
    )?;

    if env.has_errors() {
        return Err(anyhow!("Error in model building"));
    }

    let mut targets = FunctionTargetsHolder::default();

    for module_env in env.get_modules() {
        for func_env in module_env.get_functions() {
            targets.add_target(&func_env);
        }
    }

    let pipeline = default_pipeline_with_options(&options.prover);

    let mut targets = FunctionTargetsHolder::default();

    let mut temp_func_info = BTreeMap::new();
    for module_env in env.get_modules() {
        for func_env in module_env.get_functions() {
            temp_func_info.insert(func_env.data.loc, func_env.data.name);
            targets.add_target(&func_env);
        }
    }
    env.func_info = temp_func_info;
    pipeline.run(&env, &mut targets);
    Ok((env, targets))
}

pub(crate) fn prove(
    options: &Options,
    env: &GlobalEnv,
    targets: &FunctionTargetsHolder,
    round_counter: usize,
) -> Result<Vec<String>> {
    let code_writer = generate_boogie(env, &options, targets)?;
    if env.has_errors() {
        return Err(anyhow!("Error in boogie translation"));
    }

    let error_vec = verify_boogie(env, &options, targets, code_writer, round_counter)?;
    Ok(error_vec)
}

// TODO: Check these two functions to see if they return the correct value
// If this is the first time this error appears in this function
// If this mutation erases old error
pub fn reward_check_1(
    current_error: &Vec<String>,
    complete_info: &Vec<EvolutionInfo>,

    function_name: &String,
    module_name: &String,
) -> bool {
    let mut result = false;
    // check whether this is the first time the error appears

    // first get all the error messages under the function
    let mut error_message_in_function: Vec<String> = Vec::new();
    for info in complete_info {
        if info.module_id == *module_name && info.function_id == *function_name {
            error_message_in_function.extend(info.error.clone());
        }
    }
    for error in current_error {
        if !error_message_in_function.contains(&error) {
            result = true;
            break;
        }
    }
    result
}
pub fn reward_check_2(
    current_vec: &mut Vec<Option<Loc>>,
    current_error: &Vec<String>,
    complete_info: &Vec<EvolutionInfo>,
    complete_status: &BTreeMap<String, Vec<Vec<Option<Loc>>>>,
) -> bool {
    let mut result = false;
    // Find out all the Vec<Vec<>> under this function_id
    // TODO: Change the key of status file to (function_id, module_id)
    current_vec.pop().unwrap();
    let mut round_id = current_vec.len() - 1;
    let prev_vec = current_vec;
    let mut prev_error = Vec::new();
    // Find out subset -> length = len(current_vec) - 1
    // current_vec.contains(subset)

    let mut cnt = 0;
    let mut mutation_id = 0;
    for (function, mutation_vec) in complete_status {
        for item in mutation_vec {
            if item == prev_vec {
                mutation_id = cnt;
                break;
            }
            cnt = cnt + 1
        }
    }

    // find evolutioninfo according to round_id, mutation_id
    for item in complete_info {
        if round_id == item.evolution_round && mutation_id == item.mutation_id {
            prev_error = (*item.error).to_owned();
        }
    }
    for item in prev_error {
        if !current_error.contains(&item) {
            result = true;
            break;
        }
    }
    result
}

// Used to generate result report under mutation_result

// TODO: transfer this function to generate report for vec<Loc>
pub fn error_report_file_generation(
    env: &GlobalEnv,
    env_diags_map: BTreeMap<Loc, String>,
    loc_vec: Vec<Option<Loc>>,
) {
    let env_file_hash_map = &(*env).file_hash_map;
    let mut current_file_hash = loc_vec[0].unwrap().file_hash;
    let mut current_file_path = env_file_hash_map.get(&current_file_hash).unwrap().0.clone();
    let file_path_vec = current_file_path.split("/").collect::<Vec<&str>>();
    current_file_path = file_path_vec[file_path_vec.len() - 1].to_string();
    current_file_path = current_file_path[0..current_file_path.len() - 5].to_string();
    current_file_path += &"_".to_string();
    current_file_path += &"mutation.txt".to_string();
    // create the dir if not exists
    fs::create_dir_all("./mutation_result");
    current_file_path = "./mutation_result/".to_string() + &current_file_path.to_string();
    let mut file = if Path::new(&current_file_path).exists() {
        OpenOptions::new()
            .append(true)
            .open(&current_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .write(true)
            .create(true)
            .open(&current_file_path)
            .unwrap()
    };
    write!(file, "Mutation Points {:?}\n", &loc_vec);
    write!(file, "Mutation Types {:?}\n", &env.appendix);
    for wrapped_loc in loc_vec {
        let loc = wrapped_loc.unwrap();
        let diag_str_map = BTreeMap::from([
            (
                "ArithmeticOperator".to_string(),
                codes::Mutation::ArithmeticOperator,
            ),
            ("IfElse".to_string(), codes::Mutation::IfElse),
            ("BreakContinue".to_string(), codes::Mutation::ContinueBreak),
            ("Constant".to_string(), codes::Mutation::Constant),
            ("Unary".to_string(), codes::Mutation::Unary),
            ("BitOperator".to_string(), codes::Mutation::BitOperator),
            (
                "CompareOperator".to_string(),
                codes::Mutation::CompareOperator,
            ),
            ("BoolOperator".to_string(), codes::Mutation::BoolOperator),
            ("EqualOperator".to_string(), codes::Mutation::EqualOperator),
        ]);
        // if there is a mutation pass, write it into the report file
        let source_files = &(*env).files;
        let mut temp_diags = Diagnostics::new();
        let current_mutation_type = env_diags_map.get(&loc).unwrap().to_owned().to_owned();
        temp_diags.add(diag!(
            *diag_str_map.get(&current_mutation_type).unwrap(),
            (loc, "prover passed after mutation")
        ));

        let loc_result =
            diagnostics::report_diagnostics_to_buffer(source_files, temp_diags.clone());
        let loc_result_char = String::from_utf8(loc_result).unwrap();

        write!(file, "{}", &loc_result_char);
    }
}

pub fn genesis_set_generation(
    current_function_name: String,
    mut mutation_status: BTreeMap<String, Vec<Vec<Option<Loc>>>>,
    loc: Loc,
) -> BTreeMap<String, Vec<Vec<Option<Loc>>>> {
    //Genesis round

    // If this is the first time a function name shows up, insert an empty vector for it
    mutation_status
        .entry(current_function_name.clone())
        .or_insert_with(|| Vec::new());
    // insert into the dict
    let mut current_vec = mutation_status.get(&current_function_name).unwrap().clone();
    current_vec.push(vec![Some(loc)]);
    // renew the mutation_set
    mutation_status.insert(current_function_name, current_vec);

    mutation_status
}

// Generate the 2 steps mutant

// First a list for all the possible mutation types:

// this function returns whether to continue on the current branch or not
//

pub fn check_fin(current_vec: Vec<Option<Loc>>) -> bool {
    let mut round_id = current_vec.len() - 1;
    // open the status file
    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists(){
        OpenOptions::new().read(true).write(true).open(&evolution_status_file_path).unwrap()
    }else{
        OpenOptions::new().read(true).write(true).append(false).create(true).open(&evolution_status_file_path).unwrap()
    };
    let mut evolution_status_content: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file) {
        Ok(content) => evolution_status_content = content,
        Err(e) => {}
    }
    let mut mutation_id = 0;
    let mut found_flag = false;
    for (function, vec) in evolution_status_content {
        if found_flag {
            break;
        }
        for item in &vec {
            if item.len() == current_vec.len() {
                if *item == current_vec {
                    found_flag = true;
                    break;
                }
                mutation_id = mutation_id + 1;
            }
        }
    }

    // open the info file
    let mut evolution_info: Vec<EvolutionInfo> = Vec::new();

    let mut evolution_info_file = if Path::new(&evolution_info_file_path).exists() {
        OpenOptions::new()
            .read(true)
            .open(&evolution_info_file_path)
            .unwrap()
    } else {
        OpenOptions::new()
            .read(true)
            .create(true)
            .open(&evolution_info_file_path)
            .unwrap()
    };
    let reader_content = serde_json::from_reader(&evolution_info_file);
    match reader_content {
        Ok(content) => {
            evolution_info = content;
        }
        Err(e) => {
            println!("error in reading content!{:?}", &e)
        }
    };

    let mut fin_sig = false;
    // return the mutation and evolution id of the vec
    for item in evolution_info {
        if item.mutation_id == mutation_id && item.evolution_round == round_id {
            fin_sig = item.fin_sig;
            break;
        }
    }
    fin_sig
}

// this function returns the current round and mutation id
pub fn check_status(evolution_vec: Vec<EvolutionInfo>) -> (usize, usize) {
    // if the mutation process already started before, resume it
    // if not, return 0 to tell the outer loop to start

    let round_id = if evolution_vec.is_empty() {
        0
    } else {
        let last_item = evolution_vec.last().unwrap();
        last_item.evolution_round
    };
    let mutation_id = if evolution_vec.is_empty() {
        0
    } else {
        let last_item = evolution_vec.last().unwrap();
        last_item.mutation_id
    };
    (round_id, mutation_id)
}
