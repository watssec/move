// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Support for the mutation in the package system.

use anyhow::{bail,Result};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use move_package::{BuildConfig, ModelConfig};
use std::fs::OpenOptions;
use std::{
    io::Write,
    path::Path,
    time::Instant,
    ops::Range,
};
use std::convert::TryInto;
use move_ir_types::location;
use move_compiler::{ diagnostics::{self, codes, Diagnostics}};
use move_model::parse_addresses_from_options;
extern crate pbr;
use pbr::ProgressBar;
use codespan::FileId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use anyhow::anyhow;
use std::collections::BTreeMap;
use move_stackless_bytecode::{
    function_target_pipeline::FunctionTargetsHolder,
    pipeline_factory::default_pipeline_with_options,
};
use move_model::{
    model::GlobalEnv,
    options::ModelBuilderOptions,
    run_model_builder_with_options_and_compilation_flags,
};
use move_prover::{cli::Options as CliOptions, generate_boogie, verify_boogie};

use move_compiler::Flags;
use move_ir_types::location::*;
use move_prover::cli::Options;

use std::io::BufWriter;
use std::io::BufReader;
use std::fs::File;

use std::io::copy;
use std::io::stdout;
extern crate rustc_serialize;
use rustc_serialize::json::Json;
//use serde_json::Result;
// =================================================================================================
// Running the mutation as a package command

#[derive(Serialize, Deserialize, Debug)]
pub struct EvolutionEnv{
    mutation_location: location::Loc,
    evolution_round: usize,
    file_id: FileId,
    range: Range<usize>,
    message: String,
}

pub fn run_move_mutation(
    mut config: BuildConfig,
    path: &Path,
    target_filter: &Option<String>,
    for_test: bool,
    options: &[String],
) -> anyhow::Result<()> {

    let report_dir = "".to_string();
    // report file system

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



    //let res = mutation_run(args, model);
    // Instead of invoking a function, let's directly put the code here.

    let now = Instant::now();
    let mut init_flag = true;
    // return env and target from
    let fake_loc =None;


    let (env, _targets) = prepare(config.clone(), path,target_filter,&options, &init_flag, fake_loc)?;


    init_flag = false;
    let mut diags = Diagnostics::new();
    let mut cnt = 0;

    let mut mutate_loc_original = Vec::new();
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

    let env_diags_map = env.diags_map;

    for (loc, _result) in env.mutation_counter{
        // judge that the loc which is gonna to be pushed into mutate_loc_original is not one from source
        // and at the same time not one that is ignored by the user

        if *env.is_source_module.get(env.module_ident.get(&loc).unwrap()).unwrap()&& !mutate_loc_original.contains(&loc)
        {
            mutate_loc_original.push(loc);
        }
    }

    //File root path
    let mut root_path = "./mutation_result/".to_string();

    let mut bar_length = mutate_loc_original.len();
    let mut pb = ProgressBar::new(bar_length.try_into().unwrap());
    for loc in mutate_loc_original {

        use std::fmt::Write;
        let current_mutation_type = env_diags_map.get(&loc).unwrap();

        let (env, targets) = prepare(config.clone(), path, target_filter, &options, &init_flag, Some(loc))?;
        let proved = prove(&options, &env, &targets)?;
        // println!("file_hash_map {:?}",&env.file_hash_map);
        // if the mutated result passed the
        let env_file_hash_map = env.file_hash_map;
        let mut current_file_hash = loc.file_hash;
        let mut current_file_path = env_file_hash_map.get(&current_file_hash).unwrap().0.clone();
        let file_path_vec = current_file_path.split("/").collect::<Vec<&str>>();
        current_file_path = file_path_vec[file_path_vec.len()-1].to_string();
        current_file_path = current_file_path[0..current_file_path.len()-5].to_string();
        current_file_path += &"_".to_string();
        current_file_path += &"mutation.txt".to_string();
        current_file_path = "./mutation_result/".to_string()+&current_file_path.to_string();
        println!("current_file_path{:?}",&current_file_path);
        if proved {
           if env.mutated{
               // if proved, record to the report file
              println!("current_mutation_type{:?}",&current_mutation_type);
              diags.add(diag!(*diag_str_map.get(current_mutation_type).unwrap(), (loc,"prover passed after mutation")));
              //Check whether the file exists

               // if there is a mutation pass, write it into the report file
               let source_files = env.files;
               //println!("source_file{:?}",&source_files);
               let mut temp_diags = Diagnostics::new();
               temp_diags.add(diag!(*diag_str_map.get(current_mutation_type).unwrap(), (loc,"prover passed after mutation")));
               let mut file = if Path::new(&current_file_path).exists(){
                   OpenOptions::new().append(true).open(&current_file_path)?
               }else{
                   OpenOptions::new().write(true).create(true).open(&current_file_path)?
               };
               let loc_result = diagnostics::report_diagnostics_to_buffer(&source_files, temp_diags.clone());
               println!("loc_result{:?}",loc_result);
               let loc_result_char = String::from_utf8(loc_result).unwrap();
               write!(file, "{}", &loc_result_char)?;
           }
        }else {
            // if not proved, and mutated, record the error message
            if env.mutated{

                // serialize the information & record the error into a file
                let env_diags_vec = env.diags.into_inner();

                println!("env_diags{:?}",env_diags_vec);
                let genesis_round:usize = "0".parse().unwrap();
                for env_diags in env_diags_vec {
                    for label in env_diags.0.labels {
                        // There can be several labels
                        println!("file_id{:?}", &label.file_id);
                        let mut evolution_vec = json!({
                        "mutation_location": loc,
                        "evolution_round": genesis_round,
                        "file_id": label.file_id,
                        "range": label.range,
                        "message": label.message,
                    });

                        //concat json
                        // In order to get prettier print

                        let serde_env_diags = serde_json::to_string_pretty(&evolution_vec).unwrap();
                        let evolution_path = "evolution.json";
                        let mut writer = if Path::new(evolution_path).exists(){
                            OpenOptions::new().append(true).open(&evolution_path)?
                        }else{
                            OpenOptions::new().write(true).create(true).open(&evolution_path)?
                        };
                        serde_json::to_writer(writer, &serde_env_diags).unwrap();
                        //writer.write('\n').unwrap();
                        //reward_check();
                    }
                }
            }
        }

    pb.inc();

    }
    // tagging function to check whether this generation is rewarded
    // 1) explores new part of spec to be checked

    // 2) eliminate the error given by the previous generation
    // reward_check();

    pb.finish_print("done");
    let mutation_duration = now.elapsed();

    println!(
        "{:?} mutations, {:.3} seconds",
        cnt,
        mutation_duration.as_secs_f64()
    );



    Ok(())
}






// pub(crate) means the function is private within the crate
// prepare gets back the GlobalEnv and FunctionTargetsHolder
pub(crate) fn prepare(config: BuildConfig, path: &Path, target_filter: &Option<String>, options: &Options, init_flag: &bool, loc: Option<Loc>) -> Result<(GlobalEnv, FunctionTargetsHolder)> {

    let mut flags = Flags::empty();
    // if this is the init process
    if *init_flag{
        flags.mutation = false; }
    else{
        flags.mutation = true;
        let current_loc = loc.unwrap();
        flags.current_file_hash = current_loc.file_hash.to_string();
        flags.current_start = current_loc.start;
        flags.current_end = current_loc.end;
    }

    let env = config.clone().move_model_for_package(
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

    for module_env in env.get_modules() {
        for func_env in module_env.get_functions() {
            targets.add_target(&func_env);
        }
    }
    pipeline.run(&env, &mut targets);
    Ok((env, targets))
}
pub(crate) fn prove(
    options: &Options,
    env: &GlobalEnv,
    targets: &FunctionTargetsHolder,
) -> Result<bool> {
    let code_writer = generate_boogie(env, &options, targets)?;
    if env.has_errors() {
        return Err(anyhow!("Error in boogie translation"));
    }
    verify_boogie(env, &options, targets, code_writer)?;
    Ok(!env.has_errors())
}


pub fn reward_check()
{
    let mut file = File::open("").unwrap();
    let mut stdout = stdout();
    let mut str = &copy(&mut file, &mut stdout).unwrap().to_string();
    let data = Json::from_str(str).unwrap();
    println!("data{:?}",&data);


}




