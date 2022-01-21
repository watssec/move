// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use move_lang::shared::NumericalAddress;
use bytecode::{
    function_target_pipeline::FunctionTargetsHolder, options::ProverOptions,
    pipeline_factory::default_pipeline_with_options,
};
use move_model::{
    model::{GlobalEnv, VerificationScope},
    options::ModelBuilderOptions,
    run_model_builder_with_options_and_compilation_flags,
};
use move_prover::{cli::Options as CliOptions, generate_boogie, verify_boogie};
use crate::options::MutationOptions;
use move_lang::Flags;
use move_ir_types::location::*;

// pub(crate) means the function is private within the crate
// prepare gets back the GlobalEnv and FunctionTargetsHolder
pub(crate) fn prepare(options: &MutationOptions, init_flag: &bool, loc: Option<Loc>) -> Result<(GlobalEnv, FunctionTargetsHolder)> {
    let mut named_addresses = BTreeMap::new();
    if !options.no_default_named_addresses {
        let default_mapping = [
            ("Std", "0x1"),
            ("DiemFramework", "0x1"),
            ("DiemRoot", "0xA550C18"),
            ("CurrencyInfo", "0xA550C18"),
            ("TreasuryCompliance", "0xB1E55ED"),
            ("VMReserved", "0x0"),
        ];
        named_addresses.extend(
            default_mapping
                .iter()
                .map(|(name, addr)| (name.to_string(), NumericalAddress::parse_str(addr).unwrap())),
        );
    }

    let mut flags = Flags::empty();
    // if this is the init process
    if *init_flag{
    flags.mutation = false; }
    else{
        flags.mutation= true;
        let current_loc = loc.unwrap();
        flags.current_file_hash = current_loc.file_hash.to_string();
        flags.current_start = current_loc.start;
        flags.current_end = current_loc.end;
    }





    let env = run_model_builder_with_options_and_compilation_flags(
        &options.srcs,
        &options.deps,
        ModelBuilderOptions::default(),
        flags,
        named_addresses,
    )?;

    let prover_options = get_prover_options(options);
    let pipeline = default_pipeline_with_options(&prover_options);
    env.set_extension(prover_options);

    if env.has_errors() {
        return Err(anyhow!("Error in model building"));
    }

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
    options: &MutationOptions,
    env: &GlobalEnv,
    targets: &FunctionTargetsHolder,
) -> Result<bool> {
    let cli_options = get_cli_options(options);
    let code_writer = generate_boogie(env, &cli_options, targets)?;
    if env.has_errors() {
        return Err(anyhow!("Error in boogie translation"));
    }
    verify_boogie(env, &cli_options, targets, code_writer)?;
    Ok(!env.has_errors())
}

fn get_prover_options(options: &MutationOptions) -> ProverOptions {
    let verify_scope = match &options.target {
        None => VerificationScope::All,
        Some(target) => VerificationScope::Only(target.clone()),
    };
    ProverOptions {
        verify_scope,
        ..Default::default()
    }
}

fn get_cli_options(options: &MutationOptions) -> CliOptions {
    CliOptions {
        move_sources: options.srcs.clone(),
        move_deps: options.deps.clone(),
        prover: get_prover_options(options),
        ..Default::default()
    }
}

