// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
mod options;
mod workflow;
use std::io::prelude::*;
pub use options::MutationOptions;
use std::path::Path;
use move_lang::{diag, diagnostics::{self, codes,Diagnostics}};
use std::time::Instant;
use rand::{Rng, SeedableRng};
use rand::prelude::*;
extern crate pbr;
use pbr::ProgressBar;
//**************************************************************************************************
// Entrypoint
//**************************************************************************************************

pub fn run(options: &mut MutationOptions) -> Result<()> {
    // is mutation_option is empty, which means the user didn't set any limitations
    if options.mutation_option.is_empty(){
    (*options).mutation_option.push(("ArithmeticOperator".to_string(),true));
    (*options).mutation_option.push(("Constant".to_string(),true));
    (*options).mutation_option.push(("IfElse".to_string(),true));
    (*options).mutation_option.push(("ContinueBreak".to_string(),true));
    (*options).mutation_option.push(("Borrow".to_string(),true));
    }
    println!("{:?}",&options.mutation_option);
    let now = Instant::now();
    let mut init_flag = true;
    // return env and target from
    let fake_loc =None;
    let (env, _targets) = workflow::prepare(options, &init_flag, fake_loc)?;
    let mut file_path = options.srcs[0].clone();
    let file_path_vec = file_path.split("/").collect::<Vec<&str>>();
    file_path = file_path_vec[file_path_vec.len()-1].to_string();
    file_path = file_path[0..file_path.len()-5].to_string();
    let mut result_map = BTreeMap::new();
    // if the report file does not exist, create the file
    file_path += &"_".to_string();
    file_path += &"mutation.txt".to_string();
    file_path = "./mutation_result/".to_string()+&file_path.to_string();

    let mut file = if Path::new(&file_path).exists(){
        OpenOptions::new().append(true).open(&file_path)?
    }else{
        OpenOptions::new().write(true).create(true).open(&file_path)?
    };

    init_flag = false;
    // iterate through the iterator
    let diags = Diagnostics::new();
    let files = env.files;
    let mut cnt = 0;


    //let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(10);
    // create the random number generator
    //TODO: Consider changing the env.is_source_module to map<loc, bool>



    let mut rng = rand::thread_rng();
    let mut mutate_loc_original = Vec::new();
    let diag_str_map = BTreeMap::from([
        ("ArithmaticOperator".to_string(),codes::Mutation::ArithmeticOperator),
        ("IfElse".to_string(),codes::Mutation::IfElse),
        ("ContinueBreak".to_string(), codes::Mutation::ContinueBreak),
        ("Constant".to_string(),codes::Mutation::Constant),
        ("Borrow".to_string(), codes::Mutation::Borrow),
    ]);

    let mut user_filter = Vec::new();
    for (mutation_type, result) in &options.mutation_option{
        if *result == true{
            user_filter.push(mutation_type);

        }
    }
    let env_diags_map = env.diags_map;
    for (loc, _result) in env.mutation_counter{
        // judge that the loc which is gonna to be pushed into mutate_loc_original is not one from source
        // and at the same time not one that is ignored by the user
        if !env.is_source_module.get(env.module_ident.get(&loc).unwrap()).unwrap()
            && user_filter.contains(&env_diags_map.get(&loc).unwrap()){
            mutate_loc_original.push(loc);
        }
    }

    let mut mutate_loc = Vec::new();
    mutate_loc.push(*mutate_loc_original.choose(&mut rng).unwrap());

    let mut pb = ProgressBar::new(mutate_loc.len() as u64);
    for loc in mutate_loc_original {
        pb.inc();
        let current_mutation_type = env_diags_map.get(&loc).unwrap();
        let (env, targets) = workflow::prepare(options, &init_flag, Some(loc))?;
        let proved = workflow::prove(options, &env, &targets)?;
        println!("proved{:?}", &proved);
        // if the mutated result passed the
        if !proved {
            result_map.insert(loc, false);
        } else {
            result_map.insert(loc, true);
            // determine whether this has been mutated
            if env.mutated {
                //diags.add(diag!(diag_str_map.get(current_mutation_type).unwrap(), (loc,"prover passed after mutation")));
            }

        }
        cnt = cnt + 1;
        println!("the {:?} mutation", & cnt);
    }
    let mutation_duration = now.elapsed();

    println!(
        "{:?} mutations, {:.3}mutation",
        cnt,
        mutation_duration.as_secs_f64()
    );
    pb.finish_print("done");
    let loc_result = diagnostics::report_diagnostics_to_buffer(&files, diags.clone());
    let loc_result_char = String::from_utf8(loc_result).unwrap();
    write!(&mut file, "{}", &loc_result_char)?;
    // everything is OK
    Ok(())
}


