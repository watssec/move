// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Support for the mutation in the package system.

use anyhow::{bail,Result};
use move_package::{BuildConfig, ModelConfig};
use std::fs::{self, OpenOptions};
use std::{
    io::Write,
    path::Path,
    time::Instant,
};
use std::convert::TryInto;
use move_compiler::{ diagnostics::{self, codes, Diagnostics}};
extern crate pbr;
use pbr::ProgressBar;
use serde::{Deserialize, Serialize};
use anyhow::anyhow;
use std::collections::{BTreeMap, HashMap};
use move_stackless_bytecode::{
    function_target_pipeline::FunctionTargetsHolder,
    pipeline_factory::default_pipeline_with_options,
};
use move_model::{
    model::GlobalEnv,
    symbol::Symbol
};
use move_prover::{generate_boogie, verify_boogie};
use move_compiler::Flags;
use move_ir_types::location::*;
use move_prover::cli::Options;
use std::fs::File;
use std::io::copy;
use std::io::stdout;
extern crate rustc_serialize;
use rustc_serialize::json::Json;
use std::collections::HashSet;
use std::iter::FromIterator;
use move_compiler::shared::Identifier;
use move_compiler::parser::ast::FunctionName;

//use f::Result;
// =================================================================================================
// Running the mutation as a package command



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EvolutionInfo{
    function_id: String,
    module_id: String,
    evolution_round: usize,
    mutation_id: usize,
    error: Vec<String>,
    appendix: Vec<String>,
    fin_sig: bool,
}

pub fn run_move_mutation(
    mut config: BuildConfig,
    path: &Path,
    target_filter: &Option<String>,
    for_test: bool,
    options: &[String],
) -> anyhow::Result<()> {

    //run the prover in dev mode so that addresses get default assignments
    config.dev_mode = true;

    let mut args = vec!["package".to_string()];
    let mutation_toml = Path::new(&path).join("Mutation.toml");
    if mutation_toml.exists() {
        args.push(format!("--config={}", mutation_toml.to_string_lossy()));
    }
    args.extend(options.iter().cloned());
    let mut options = move_prover::cli::Options::create_from_args(&args)?;


    if !options.move_sources.is_empty() {
        bail!(
            "move prover options must not specify sources as those are given \
                     by the package system. Did you meant to prefix `{}` with `-t`?",
            &options.move_sources[0]
        );
    }
    if !options.move_deps.is_empty() {
        bail!(
            "move prover options must not specify dependencies as those are given \
                     by the package system"
        );
    }
    if !options.move_named_address_values.is_empty() {
        bail!(
            "move prover options must not specify named addresses as those are given \
                     by the package system"
        );
    }

    let now = Instant::now();
    let mut init_flag = true;
    let fake_loc =vec!(None);

    /// create dict (function -> vec[mutation1, mutation2....])
    ///   -> mutation1 -> Vec<Loc>

    let evolution_status_file_path = "evolution_status.json";
    let evolution_info_file_path = "evolution_info.json";
    let mutated_file_path = "mutated_loc.json";



    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists(){
        OpenOptions::new().read(true).write(true).open(&evolution_status_file_path).unwrap()
    }else{
        OpenOptions::new().read(true).write(true).append(false).create(true).open(&evolution_status_file_path).unwrap()
    };
    let mut evolution_status_content: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file)
    {
        Ok(content) => evolution_status_content = content,
        Err(e) =>{},
    }



    // construct evolution_status_vec
    // to locate how far the mutation has reached
    let mut evolution_status_vec:Vec<Vec<Option<Loc>>> = Vec::new();
    for (function, vec) in evolution_status_content{
        for vec_item in vec{
            evolution_status_vec.push(vec_item.clone());
        }
    }



    let mut init_evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();


    let (mut env, _targets) = prepare(config.clone(), path, target_filter, &options, &init_flag, fake_loc)?;

    init_flag = false;
    let mut cnt = 0;

    let env_diags_map = env.diags_map;

    let mut mutate_loc_original: Vec<Option<Loc>> = Vec::new();
    for (loc, _result) in env.mutation_counter{

        // judge that the loc which is gonna to be pushed into mutate_loc_original is not one from source
        if *env.is_source_module.get(env.module_ident.get(&loc).unwrap()).unwrap()&& !mutate_loc_original.contains(&Some(loc))
        {
            mutate_loc_original.push(Some(loc));
        }
    }

    let mut bar_length = mutate_loc_original.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");

    // Genesis iteration
    //     -> arrange set by function name
    let mut mutation_id = 0;
    let mut genesis_evolution_status = init_evolution_status;


    // ⬇️ seldom any circumstances... The previous version is caused by a file hash problem created by me.
    // TODO: investigate under what circumstance will the loc in mutate_loc_original not be mutated
    for wrapped_loc in mutate_loc_original.clone() {

        // if the loc is in status -> it has already been mutated

        let mut loc = wrapped_loc.unwrap();
        let vec_loc = vec![Some(loc)];
        if evolution_status_vec.contains(&vec_loc) {
            continue
        }
        let (mut env, targets) = prepare(config.clone(), path, target_filter, &options, &init_flag, vec_loc.clone())?;

        env.current_vec = vec![Some(loc.clone())];
        env.genesis_flag = true;
        let mut current_function_name = String::new();

        match env.current_function {
            None => {
                println!("None;(");
                continue
            },
            Some(function_name) =>
                current_function_name = function_name.value().as_str().to_owned(),
        };

        // TODO: solve this counter in prove fun


        if env.mutated {
            // push mutated item into mutated_vec file
            // Some locs are filtered as some are not mutated
            // Some are not within a function

            // first read file
            let mut mutated_vec = Vec::new();
            let mut mutated_loc_file = if Path::new(&mutated_file_path).exists() {
                OpenOptions::new().read(true).write(true).open(&mutated_file_path).unwrap()
            } else {
                OpenOptions::new().read(true).write(true).append(false).create(true).open(&mutated_file_path).unwrap()
            };

            match serde_json::from_reader(&mutated_loc_file)
            {
                Ok(content) => {
                    mutated_vec = content;
                },
                // Error means this file has not been created yet, so we don't need to do anything
                // as the vec has already been initialized
                Err(e) => {},
            }
            // then push the current loc into the vec
            mutated_vec.push(loc);
            // then write the current vec into the file
            fs::remove_file(&mutated_file_path);
            let mutated_loc_file = OpenOptions::new().read(true).write(true).create(true).open(&mutated_file_path).unwrap();

            let serde_mutated_loc = serde_json::to_value(mutated_vec).unwrap();
            serde_json::to_writer_pretty(&mutated_loc_file, &serde_mutated_loc)?;

            let current_function_name = env.current_function.unwrap().value().as_str().to_owned();

            // this set should be generated from json file

            genesis_evolution_status = genesis_set_generation(current_function_name, genesis_evolution_status, loc);
            let pretty_genesis_evolution_status = serde_json::to_value(&genesis_evolution_status).unwrap();

            // delete the file first before write a new one
            fs::remove_file(&evolution_status_file_path)?;

            evolution_status_file =
                OpenOptions::new().read(true).write(true).create(true).open(&evolution_status_file_path).unwrap();

            serde_json::to_writer_pretty(&evolution_status_file, &pretty_genesis_evolution_status)?;
        }
        pb.inc();
    }
    // Open the status file and get the keys
    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists(){
        OpenOptions::new().read(true).write(true).open(&evolution_status_file_path).unwrap()
    }else{
        OpenOptions::new().read(true).write(true).append(false).create(true).open(&evolution_status_file_path).unwrap()
    };
    let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file)
    {
        Ok(content) => {
            evolution_status = content},
        Err(e) =>{},
    }
    let evolution_status_keys:Vec<String> = evolution_status.clone().into_keys().collect();
    let mut bar_length = evolution_status_keys.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    pb.format("╢▌▌░╟");
    for key in evolution_status_keys {
        let vec_vec_loc = evolution_status.get(&key).unwrap();
        for vec_loc in vec_vec_loc.clone(){

        let (mut env, targets) = prepare(config.clone(), path, target_filter, &options, &init_flag, vec_loc.clone())?;
        let current_function_name = env.current_function.unwrap().value().as_str().to_owned();
        let current_module_name = env.current_module.unwrap().value().as_str().to_owned();
        let current_appendix = env.appendix.clone();

        let mut evolution_info = EvolutionInfo {
            function_id: current_function_name,
            module_id: current_module_name,
            evolution_round: 0,
            mutation_id: mutation_id,
            error: vec![],
            appendix: current_appendix,
            fin_sig: false,
        };

        mutation_id = mutation_id + 1;
        let genesis_round_counter: usize = 0;
        // read the serde vec and then rewrite it when it's not empty (this should be a vector)
        let mut original_evolution_info: Vec<EvolutionInfo> = Vec::new();
        let error_vec = prove(&options, &env, &targets, genesis_round_counter)?;
        let mut evolution_info_file = if Path::new(&evolution_info_file_path).exists() {
            OpenOptions::new().read(true).write(true).open(&evolution_info_file_path).unwrap()
        } else {
            OpenOptions::new().read(true).write(true).create(true).open(&evolution_info_file_path).unwrap()
        };
        let reader_content = serde_json::from_reader(&evolution_info_file);
        match reader_content
        {
            Ok(content) => {
                original_evolution_info = content;
            },
            Err(e) => {
                println!("error in reading content!{:?}", &e)
            },
        }
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
        let mut evolution_info_file =
            OpenOptions::new().read(true).write(true).create(true).open(&evolution_info_file_path).unwrap();
        serde_json::to_writer_pretty(&evolution_info_file, &serde_evolution_info)?;
        }
        pb.inc();
    }


        // if the mutated result passed the prover
        // 1) -> label it as passed
        // 2) -> record it into files when necessary




    pb.finish_print("Genesis iteration done");



    // Evolution round:

    // for every round of mutation evolution
    //        -> for every combination within this round, mutate

    // I will set the condition to stop evolution for 2 temporarily
    // It should be...

    // Open evolution_status file first





    // open the file and get the mutated_vector

    let mutated_loc_file = OpenOptions::new().read(true).write(true).open(&mutated_file_path).unwrap();
    let mut mutated_vec:Vec<Loc> = Vec::new();
    match serde_json::from_reader(&mutated_loc_file)
    {
        Ok(content) => mutated_vec = content,
        // Error means this file has not been created yet, so we don't need to do anything
        // as the vec has already been initialized
        Err(e) =>{},
    }




    for i in 0..2{

        // update the mutation_set on every round of evolution
        // evolution_status -> vec<vec<>>  this inner vec has an order

        // open the file to get evolution_status
        let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists(){
            OpenOptions::new().read(true).write(true).open(&evolution_status_file_path).unwrap()
        }else{
            OpenOptions::new().read(true).write(true).append(false).create(true).open(&evolution_status_file_path).unwrap()
        };
        let mut evolution_status: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

        match serde_json::from_reader(&evolution_status_file)
        {
            Ok(content) => {
                evolution_status = content},
            Err(e) =>{},
        }

        evolution_status = normal_set_generation(evolution_status.clone(), mutated_vec.clone(), env.function_map.clone(),i);

        fs::remove_file(&evolution_status_file_path);

        let evolution_status_file = OpenOptions::new().read(true).write(true).create(true).open(&evolution_status_file_path).unwrap();
        let pretty_evolution_status = serde_json::to_value(&evolution_status).unwrap();
        serde_json::to_writer_pretty(&evolution_status_file, &pretty_evolution_status)?;

        // put into a file

        let mut mutation_id = 0;
        for (function, mutation_vec) in evolution_status.clone(){
            let mut evolution_bar_length = mutation_vec.len();
            let mut pb = ProgressBar::new(evolution_bar_length.try_into().unwrap());
            pb.format("╢▌▌░╟");
            for vec in mutation_vec{
                // i = 0 -> round 1 -> vec.len() >=2

                if vec.len() !=i+2{
                    continue
                }

                mutation_id = mutation_id +1;

                let (mut env, targets) = prepare(config.clone(), path, target_filter, &options, &init_flag, vec.clone())?;

                env.current_vec = vec.clone();
                env.genesis_flag = false;

                let current_function_name = env.current_function.unwrap().value().as_str().to_owned();
                let current_module_name = env.current_module.unwrap().value().as_str().to_owned();

                let current_appendix = env.appendix.clone();
                let mut evolution_info = EvolutionInfo{
                    module_id: current_module_name.clone(),
                    function_id: current_function_name.clone(),
                    evolution_round: i+1,
                    mutation_id: mutation_id-1,
                    error: vec![],
                    appendix: current_appendix,
                    fin_sig: false,
                };

                let mut evolution_info_file =
                    OpenOptions::new().read(true).write(true).open(&evolution_info_file_path).unwrap();
                let mut original_evolution_info = Vec::new();
                match serde_json::from_reader(&evolution_info_file)
                {
                    Ok(content) => original_evolution_info = content,
                    Err(e) =>{},
                }

                let error_vec = prove(&options, &env, &targets,i+1)?;
                if error_vec.is_empty(){
                    evolution_info.fin_sig = true;
                    error_report_file_generation(&env, env_diags_map.clone(), vec.clone());
                    original_evolution_info.push(evolution_info.clone());

                }else{
                    // returns true when new error is discovered
                    let result1 = reward_check_1( &error_vec, &original_evolution_info,
                                                  &current_function_name, &current_module_name);

                    // returns true when old message is overwritten
                    let mut clone_vec = vec.clone();
                    let result2 = reward_check_2(&mut clone_vec, &error_vec,
                                                 &original_evolution_info, &evolution_status);

                    // if
                    if result1 || result2 {
                        evolution_info.fin_sig = false;
                    }else{
                        evolution_info.fin_sig = true;
                    }

                    evolution_info.error = error_vec.clone();

                    original_evolution_info.push(evolution_info.clone());
                };

                fs::remove_file(&evolution_info_file_path);
                let evolution_info_file = OpenOptions::new().read(true).write(true).create(true).open(&evolution_info_file_path).unwrap();
                let serde_evolution_info = serde_json::to_value(original_evolution_info).unwrap();
                serde_json::to_writer_pretty(&evolution_info_file, &serde_evolution_info)?;
                pb.inc();
            };
        };

    };



    Ok(())
}





// tagging function to check whether this generation is rewarded
// 1) explores new part of spec to be checked

// 2) eliminate the error given by the previous generation
//  => record the error information to the json chain



//println!(
//"{:?} mutations, {:.3} seconds",
//cnt,
//mutation_duration.as_secs_f64()
//);



// pub(crate) means the function is private within the crate
// prepare gets back the GlobalEnv and FunctionTargetsHolder


pub(crate) fn prepare(config: BuildConfig, path: &Path, target_filter: &Option<String>,
                      options: &Options, init_flag: &bool, loc_vec: Vec<Option<Loc>>) ->
                      Result<(GlobalEnv, FunctionTargetsHolder)> {


    let mut flags = Flags::empty();
    // if this is the init process
    if *init_flag{
        flags.mutation = false; }
    else{
        flags.mutation = true;

        for loc in loc_vec{

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
            temp_func_info.insert(func_env.data.loc,func_env.data.name);
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
    current_error: &Vec<String>, complete_info:&Vec<EvolutionInfo>,

    function_name: &String, module_name: &String) -> bool
{
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
            break
        }
    }
    result
}
pub fn reward_check_2(current_vec: &mut Vec<Option<Loc>>,
                      current_error: &Vec<String>, complete_info:&Vec<EvolutionInfo>,
                      complete_status: &BTreeMap<String, Vec<Vec<Option<Loc>>>>) -> bool
{
    let mut result = false;
    // Find out all the Vec<Vec<>> under this function_id
    // TODO: Change the key of status file to (function_id, module_id)
    current_vec.pop().unwrap();
    let mut round_id = current_vec.len() -1;
    let prev_vec = current_vec;
    let mut prev_error = Vec::new();
    // Find out subset -> length = len(current_vec) - 1
    // current_vec.contains(subset)


    let mut cnt = 0;
    let mut mutation_id = 0;
    for (function, mutation_vec) in complete_status {
        for item in mutation_vec{

            if item == prev_vec{
                mutation_id = cnt;
                break
            }
            cnt = cnt +1
        }
    }


    // find evolutioninfo according to round_id, mutation_id
    for item in complete_info{
        if round_id == item.evolution_round && mutation_id == item.mutation_id{
            prev_error = (*item.error).to_owned();
        }
    }
    for item in prev_error{
        if !current_error.contains(&item){
            result = true;
            break
        }
    }
    result
}




// Used to generate result report under mutation_result

// TODO: transfer this function to generate report for vec<Loc>
pub fn error_report_file_generation(env: &GlobalEnv, env_diags_map: BTreeMap<Loc, String>, loc_vec: Vec<Option<Loc>>) {

    let env_file_hash_map = &(*env).file_hash_map;
    let mut current_file_hash = loc_vec[0].unwrap().file_hash;
    let mut current_file_path = env_file_hash_map.get(&current_file_hash).unwrap().0.clone();
    let file_path_vec = current_file_path.split("/").collect::<Vec<&str>>();
    current_file_path = file_path_vec[file_path_vec.len()-1].to_string();
    current_file_path = current_file_path[0..current_file_path.len()-5].to_string();
    current_file_path += &"_".to_string();
    current_file_path += &"mutation.txt".to_string();
    // create the dir if not exists
    fs::create_dir_all("./mutation_result");
    current_file_path = "./mutation_result/".to_string()+&current_file_path.to_string();
    let mut file = if Path::new(&current_file_path).exists(){
        OpenOptions::new().append(true).open(&current_file_path).unwrap()
    }else{
        OpenOptions::new().write(true).create(true).open(&current_file_path).unwrap()
    };
    write!(file, "Mutation Points {:?}\n", &loc_vec);
    write!(file, "Mutation Types {:?}\n", &env.appendix);
    for wrapped_loc in loc_vec{
        let loc = wrapped_loc.unwrap();
        let diag_str_map = BTreeMap::from([
            ("ArithmeticOperator".to_string(),codes::Mutation::ArithmeticOperator),
            ("IfElse".to_string(),codes::Mutation::IfElse),
            ("BreakContinue".to_string(), codes::Mutation::ContinueBreak),
            ("Constant".to_string(),codes::Mutation::Constant),
            ("Unary".to_string(), codes::Mutation::Unary),
            ("BitOperator".to_string(), codes::Mutation::BitOperator),
            ("CompareOperator".to_string(), codes::Mutation::CompareOperator),
            ("BoolOperator".to_string(), codes::Mutation::BoolOperator),
            ("EqualOperator".to_string(), codes::Mutation::EqualOperator)
        ]);
        // if there is a mutation pass, write it into the report file
        let source_files = &(*env).files;
        let mut temp_diags = Diagnostics::new();
        //println!("env_diags_map{:?}",&env_diags_map);
        let current_mutation_type = env_diags_map.get(&loc).unwrap().to_owned().to_owned();
        temp_diags.add(diag!(*diag_str_map.get(&current_mutation_type).unwrap(), (loc,"prover passed after mutation")));

        let loc_result = diagnostics::report_diagnostics_to_buffer(source_files, temp_diags.clone());
        let loc_result_char = String::from_utf8(loc_result).unwrap();
        write!(file, "{}", &loc_result_char);

    }
}



pub fn genesis_set_generation
(current_function_name: String,
 mut mutation_status: BTreeMap<String, Vec<Vec<Option<Loc>>>>,
 loc:Loc)
 -> BTreeMap<String, Vec<Vec<Option<Loc>>>>
{


    //Genesis round

    // If this is the first time a function name shows up, insert an empty vector for it
    mutation_status.entry(current_function_name.clone()).or_insert_with(|| Vec::new());
    // insert into the dict
    let mut current_vec = mutation_status.get(&current_function_name).unwrap().clone();
    current_vec.push(vec![Some(loc)]);
    // renew the mutation_set
    mutation_status.insert(current_function_name, current_vec);

    mutation_status
}


// Generate the new evolution of mutation set
pub fn normal_set_generation(mut mutation_status: BTreeMap<String, Vec<Vec<Option<Loc>>>>,
                             mutate_loc_original: Vec<Loc>, function_map: BTreeMap<Loc,Option<FunctionName>>, round_id: usize)
                             -> BTreeMap<String, Vec<Vec<Option<Loc>>>>
{

    let function_keys:Vec<String> = mutation_status.clone().into_keys().collect();
    // iterate through the functions

    for key in function_keys {

        let str_key = key.as_str();
        // initialize the new evolution set
        let mut new_vec: Vec<Vec<Option<Loc>>> = Vec::new();
        // for every set in the vector
        // if this vec is labelled FIN skip it
        // TODO: If 3rd+ evolution need to be supported, this continue condition
        // should be changed to !=
        for vec in mutation_status.get(str_key).unwrap().clone(){
            // check here
            let mut fin_flag = check_fin(vec.to_owned());

            // push vec into new_vec
            new_vec.push(vec.to_owned().clone());
            if fin_flag{
                continue
            }
            if vec.len() < round_id+1{
                continue
            }

            // this brings repetition
            // step0: turn vec into set
            let mut current_set = HashSet::new();
            for item in vec.clone(){
                current_set.insert(item);
            }
            // step 0.5 create a hashset for mutate_loc_original
            let mut mutate_loc_original_set = HashSet::new();
            for item in mutate_loc_original.clone(){
                // add a condition -> under the same function

                let item_function_name: String = function_map.get(&item).unwrap().unwrap().value().as_str().to_owned();
                if item_function_name == key{
                if !check_fin(vec![Some(item.clone())]){
                    mutate_loc_original_set.insert(Some(item));
                }
                }
            }
            // step1: get a sub hashset

            let mut sub_set: HashSet<Option<Loc>> = &mutate_loc_original_set- &current_set ;

            // step2: append one item from the sub hashset into
            for add in sub_set{
                let mut current_vec = vec.clone();
                current_vec.push(add);
                new_vec.push(current_vec);
            }
        }

        // prune when subset
        let mut retain_list = Vec::new();
        for i in 0..new_vec.len(){
            retain_list.push(true);
        }

        let mut repetition = Vec::new();
        let mut repetition_set = Vec::new();
        for item_outer in new_vec.clone(){
            for item_inner in new_vec.clone(){
                let item_outer_set: HashSet<Option<Loc>> = HashSet::from_iter(item_outer.clone());
                let item_inner_set: HashSet<Option<Loc>> = HashSet::from_iter(item_inner.clone());
                let index_outer = new_vec.iter().position(|x| *x == item_outer).unwrap();
                let index_inner = new_vec.iter().position(|x| *x == item_inner).unwrap();
                //Set<a,b> and Set<b,a> are still considered as two vecs..
                if item_inner_set.is_subset(&item_outer_set) && item_inner_set != item_inner_set{
                    let index_outer = new_vec.iter().position(|x| *x == item_outer).unwrap();
                    retain_list[index_outer] = false;
                }

                //Two situations -> the same item/ set-wise same item
                if  item_inner_set == item_outer_set{
                    // not the same item
                    if index_outer != index_inner{
                        let mut temp_set = HashSet::new();
                        temp_set.insert(index_inner);
                        temp_set.insert(index_outer);

                        if !repetition_set.contains(&temp_set){
                            repetition.push(vec![index_inner,index_outer]);
                            repetition_set.push(temp_set);
                        };

                    }
                }
            }
        }
        for item in repetition{
            retain_list[item[0]] = false;
        }
        let mut iter = retain_list.iter();
        new_vec.retain(|_| *iter.next().unwrap());
        mutation_status.insert(key.to_string(), new_vec.clone());
    }
    mutation_status
}


// this function returns whether to continue on the current branch or not
pub fn check_fin (current_vec: Vec<Option<Loc>>)
                  -> bool {

    let evolution_status_file_path = "evolution_status.json";
    let evolution_info_file_path = "evolution_info.json";
    let mut round_id = current_vec.len() - 1;
    // open the status file
    let mut evolution_status_file = if Path::new(&evolution_status_file_path).exists(){
        OpenOptions::new().read(true).open(&evolution_status_file_path).unwrap()
    }else{
        OpenOptions::new().read(true).append(false).create(true).open(&evolution_status_file_path).unwrap()
    };
    let mut evolution_status_content: BTreeMap<String, Vec<Vec<Option<Loc>>>> = BTreeMap::new();

    match serde_json::from_reader(&evolution_status_file)
    {
        Ok(content) => evolution_status_content = content,
        Err(e) =>{},
    }
    let mut mutation_id = 0;
    let mut found_flag = false;
    for (function, vec) in evolution_status_content{
        if found_flag{break}
        for item in &vec{
            if item.len() == current_vec.len(){
                if *item == current_vec{
                    found_flag =true;
                    break
                }
                mutation_id = mutation_id +1;
            }
        }
    };

    // open the info file
    let mut evolution_info: Vec<EvolutionInfo> = Vec::new();

    let mut evolution_info_file = if Path::new(&evolution_info_file_path).exists() {
        OpenOptions::new().read(true).open(&evolution_info_file_path).unwrap()
    } else {
        OpenOptions::new().read(true).create(true).open(&evolution_info_file_path).unwrap()
    };
    let reader_content = serde_json::from_reader(&evolution_info_file);
    match reader_content
    {
        Ok(content) => {
            evolution_info = content;
        },
        Err(e) => {
            println!("error in reading content!{:?}", &e)
        },
    };
    let mut fin_sig = false;
    // return the mutation and evolution id of the vec
    for item in evolution_info{
        if item.mutation_id == mutation_id && item.evolution_round == round_id{
            fin_sig = item.fin_sig;
            break
        }
    };
    fin_sig

}

// this function returns the current round and mutation id
pub fn check_status(evolution_vec: Vec<EvolutionInfo>) -> (usize, usize){
    // if the mutation process already started before, resume it
    // if not, return 0 to tell the outer loop to start


    let round_id = if evolution_vec.is_empty(){
        0
    }else{
        let last_item = evolution_vec.last().unwrap();
        last_item.evolution_round
    };
    let mutation_id = if evolution_vec.is_empty() {
        0
    }else{
        let last_item = evolution_vec.last().unwrap();
        last_item.mutation_id
    };
    (round_id, mutation_id)
}

