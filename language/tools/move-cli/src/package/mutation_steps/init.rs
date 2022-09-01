use crate::package::mutation::*;
use crate::package::mutation_steps::utils::*;
use anyhow::{anyhow, bail, Result};
use move_compiler::parser::ast::FunctionName;
use move_compiler::shared::Identifier;
use move_ir_types::location::*;
use move_package::{BuildConfig, ModelConfig};
use move_prover::cli::Options;
use pbr::ProgressBar;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::fs::{self, OpenOptions};
use std::path::Path;

pub fn run_mutation_init(
    path: &Path,
    mut config: BuildConfig,
    target_filter: &Option<String>,
    vec_options: &Options,
) -> anyhow::Result<()> {
    println!("here!");
    let mut init_flag = true;
    let fake_loc = vec![None];

    /// create dict (function -> vec[mutation1, mutation2....])
    ///   -> mutation1 -> Vec<Loc>
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
    let mut evolution_status_content: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file) {
        Ok(content) => evolution_status_content = content,
        Err(e) => {}
    }

    // construct evolution_status_vec
    // to locate how far the mutation has reached
    let mut evolution_status_vec: Vec<Vec<Option<Loc>>> = Vec::new();
    for (function, vec) in evolution_status_content {
        for vec_item in vec {
            evolution_status_vec.push(vec_item.clone());
        }
    }

    let mut init_evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();
    let (mut env, _targets) = prepare(
        config.clone(),
        path,
        target_filter,
        &vec_options,
        &init_flag,
        fake_loc,
    )?;

    init_flag = false;
    let mut cnt = 0;

    let env_diags_map = env.diags_map;
    let env_function_map = env.function_map;
    let mut mutate_loc_original: Vec<Option<Loc>> = Vec::new();
    for (loc, _result) in env.mutation_counter {
        // judge that the loc which is gonna to be pushed into mutate_loc_original is not one from source
        if *env
            .is_source_module
            .get(env.module_ident.get(&loc).unwrap())
            .unwrap()
            && !mutate_loc_original.contains(&Some(loc))
        {
            mutate_loc_original.push(Some(loc));
        }
    }

    let mutate_loc_original_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&mutate_loc_original_file_path)
        .unwrap();

    let serde_mutated_loc = serde_json::to_value(&mutate_loc_original).unwrap();
    serde_json::to_writer_pretty(&mutate_loc_original_file, &serde_mutated_loc)?;

    let mut bar_length = mutate_loc_original.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");

    // Genesis iteration
    //     -> arrange set by function name

    let mut genesis_evolution_status = init_evolution_status;
    for wrapped_loc in mutate_loc_original.clone() {
        // if the loc is in status -> it has already been mutated

        let mut loc = wrapped_loc.unwrap();
        let vec_loc = vec![Some(loc)];
        if evolution_status_vec.contains(&vec_loc) {
            continue;
        }
        let (mut env, targets) = prepare(
            config.clone(),
            path,
            target_filter,
            &vec_options,
            &init_flag,
            vec_loc.clone(),
        )?;

        env.current_vec = vec![Some(loc.clone())];
        env.genesis_flag = true;
        let mut current_function_name = String::new();

        match env.current_function {
            None => {
                println!("None;(");
                continue;
            }
            Some(function_name) => {
                current_function_name = function_name.value().as_str().to_owned()
            }
        };

        // TODO: solve this counter in prove fun

        if env.mutated {
            // push mutated item into mutated_vec file
            // Some locs are filtered as some are not mutated
            // Some are not within a function

            // first read file
            let mut mutated_vec = Vec::new();
            let mut mutated_loc_file = if Path::new(&mutated_file_path).exists() {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&mutated_file_path)
                    .unwrap()
            } else {
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .append(false)
                    .create(true)
                    .open(&mutated_file_path)
                    .unwrap()
            };

            match serde_json::from_reader(&mutated_loc_file) {
                Ok(content) => {
                    mutated_vec = content;
                }
                // Error means this file has not been created yet, so we don't need to do anything
                // as the vec has already been initialized
                Err(e) => {}
            }
            // then push the current loc into the vec
            mutated_vec.push(loc);
            // then write the current vec into the file
            fs::remove_file(&mutated_file_path);
            let mutated_loc_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&mutated_file_path)
                .unwrap();

            let serde_mutated_loc = serde_json::to_value(mutated_vec).unwrap();
            serde_json::to_writer_pretty(&mutated_loc_file, &serde_mutated_loc)?;

            let current_function_name = env.current_function.unwrap().value().as_str().to_owned();

            // this set should be generated from json file

            genesis_evolution_status =
                genesis_set_generation(current_function_name, genesis_evolution_status, loc);
            let pretty_genesis_evolution_status =
                serde_json::to_value(&genesis_evolution_status).unwrap();

            // delete the file first before write a new one
            fs::remove_file(&evolution_status_file_path)?;

            evolution_status_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&evolution_status_file_path)
                .unwrap();

            serde_json::to_writer_pretty(&evolution_status_file, &pretty_genesis_evolution_status)?;
        }
        pb.inc();
    }

    let mut diags_vec: Vec<String> = Vec::new();
    for (loc, diags) in env_diags_map {
        diags_vec.push(diags);
    }
    let mut env_diags_map_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&env_diags_map_file_path)
        .unwrap();
    let pretty_env_diags_map = serde_json::to_value(&diags_vec).unwrap();
    serde_json::to_writer_pretty(&env_diags_map_file, &pretty_env_diags_map)?;

    let mut function_vec: Vec<Option<FunctionName>> = Vec::new();
    for (loc, function) in env_function_map {
        function_vec.push(function)
    }
    let mut env_function_map_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&env_function_map_file_path)
        .unwrap();
    let pretty_env_function_map = serde_json::to_value(&function_vec).unwrap();
    serde_json::to_writer_pretty(&env_function_map_file, &pretty_env_function_map)?;
    Ok(())
}
